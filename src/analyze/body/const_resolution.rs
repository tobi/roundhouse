//! Ruby constant references indexed from the application's Ruby sources.
//!
//! Rubydex resolves Ruby lexical names once, then releases its graph.
//! Roundhouse retains source-position answers and keys inferred values
//! by Rubydex declaration IDs.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;

use rubydex::indexing::local_graph::LocalGraph;
use rubydex::indexing::{IndexerBackend, LanguageId, build_local_graph};
use rubydex::model::declaration::Declaration;
use rubydex::model::definitions::Definition;
use rubydex::model::graph::Graph;
use rubydex::model::identity_maps::IdentityHashMap;
use rubydex::model::ids::{DeclarationId, UriId};
use rubydex::resolution::Resolver;

use crate::ident::{ClassId, Symbol};
use crate::span::{FileId, SourceFile, Span};

// Rubydex's minimal built-ins stop at Object/Module/Class. These are
// Ruby core classes already modeled by Roundhouse's primitive dispatch,
// declared as RBS so the source graph resolves them by Ruby name.
const CORE_RBS: &str = "\
class Numeric < Object\nend\n\
class Integer < Numeric\nend\n\
class Float < Numeric\nend\n\
class String < Object\nend\n\
class Array < Object\nend\n\
class Hash < Object\nend\n\
class Symbol < Object\nend\n\
class TrueClass < Object\nend\n\
class FalseClass < Object\nend\n\
class NilClass < Object\nend\n\
class Regexp < Object\nend\n\
class Exception < Object\nend\n\
class StandardError < Exception\nend\n";

const CORE_URI: &str = "roundhouse-core:rbs";
const RUNTIME_URI_PREFIX: &str = "roundhouse-runtime:";

pub(super) enum ResolvedConstant {
    Namespace { class: Arc<ClassId>, runtime: bool },
    Value(DeclarationId),
}

type Answers = HashMap<(FileId, u32), Option<ResolvedConstant>>;

pub(crate) struct ConstResolver {
    references: Answers,
    indexed_files: Vec<bool>,
}

/// One Rubydex input document. The thread that indexes it builds the
/// URI, so each URI costs one allocation.
struct Document<'a> {
    uri_prefix: &'static str,
    path: &'a str,
    text: &'a str,
    language: LanguageId,
}

impl Document<'_> {
    fn index(&self) -> LocalGraph {
        let uri = [self.uri_prefix, self.path].concat();
        build_local_graph(uri, self.text, &self.language, IndexerBackend::RubyIndexer)
    }
}

/// The caller thread merges local graphs one at a time. On an app with
/// 60 MB of Ruby, more than 8 parsers made the merge no faster and held
/// more parse memory.
#[cfg(not(target_arch = "wasm32"))]
const MAX_INDEX_WORKERS: usize = 8;

/// Rubydex parses each document independently, so workers build the
/// local graphs in parallel. This thread merges them in input order, so
/// the graph does not depend on thread timing. Every URI is new in this
/// analysis: a bulk merge skips incremental invalidation.
#[cfg(not(target_arch = "wasm32"))]
fn index_documents(documents: &[Document<'_>]) -> Graph {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(MAX_INDEX_WORKERS)
        .min(documents.len());
    let next = AtomicUsize::new(0);
    let mut graph = Graph::new();
    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        for _ in 0..workers {
            let sender = sender.clone();
            let next = &next;
            std::thread::Builder::new()
                .name("rubydex-index".into())
                // The indexer recurses through each expression, like the
                // compiler walkers, so it gets the same stack budget.
                .stack_size(crate::stack::COMPILER_STACK_BYTES)
                .spawn_scoped(scope, move || {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(document) = documents.get(index) else { break };
                        if sender.send((index, document.index())).is_err() {
                            break;
                        }
                    }
                })
                .expect("spawn a Rubydex indexing thread");
        }
        drop(sender);
        let mut pending = BTreeMap::new();
        let mut merged = 0;
        for (index, local) in receiver {
            pending.insert(index, local);
            while let Some(local) = pending.remove(&merged) {
                graph.extend(local);
                merged += 1;
            }
        }
    });
    graph
}

/// The browser build has no threads.
#[cfg(target_arch = "wasm32")]
fn index_documents(documents: &[Document<'_>]) -> Graph {
    let mut graph = Graph::new();
    for document in documents {
        graph.extend(document.index());
    }
    graph
}

fn is_runtime_declaration(graph: &Graph, declaration: &Declaration) -> bool {
    declaration.definitions().iter().any(|id| {
        graph
            .definitions()
            .get(id)
            .and_then(|definition| graph.documents().get(definition.uri_id()))
            .is_some_and(|document| {
                document.uri() == CORE_URI || document.uri().starts_with(RUNTIME_URI_PREFIX)
            })
    })
}

/// Rubydex promotes `X = <method call>` to a module when code calls a
/// method on `X`, because the call could build a class (`Struct.new`).
/// Roundhouse types the assigned value instead. A `class` or `module`
/// definition, including `Class.new` and `Module.new`, keeps the namespace.
fn is_assigned_value(graph: &Graph, declaration: &Declaration) -> bool {
    let mut assigned = false;
    for id in declaration.definitions() {
        match graph.definitions().get(id) {
            Some(Definition::Constant(_)) => assigned = true,
            Some(Definition::Class(_) | Definition::Module(_) | Definition::SingletonClass(_)) => {
                return false;
            }
            _ => {}
        }
    }
    assigned
}

impl ConstResolver {
    /// Index real Ruby app sources with their original 1-based file IDs.
    pub(crate) fn from_app_sources(sources: &[SourceFile]) -> Self {
        let indexed_files: Vec<bool> =
            sources.iter().map(|source| source.path.ends_with(".rb")).collect();
        let mut graph =
            crate::timings::phase("rubydex: index", || index_sources(sources, &indexed_files));
        // Rubydex resolves all source references after every file enters
        // its graph, including declarations in later files.
        crate::timings::phase("rubydex: resolve", || Resolver::new(&mut graph).resolve());
        let references = crate::timings::phase("rubydex: answers", || {
            collect_answers(&graph, sources, &indexed_files)
        });
        crate::timings::phase("rubydex: release graph", || drop(graph));
        Self {
            references,
            indexed_files,
        }
    }

    pub(super) fn has_source_file(&self, file: FileId) -> bool {
        file.0
            .checked_sub(1)
            .and_then(|index| self.indexed_files.get(index as usize))
            .copied()
            .unwrap_or(false)
    }

    /// IR spans cover the entire `A::B::C` path. Rubydex records the
    /// final name at the span end minus the name's byte length.
    pub(super) fn reference(
        &self,
        span: Span,
        path: &[Symbol],
    ) -> Option<Option<&ResolvedConstant>> {
        let length = u32::try_from(path.last()?.as_str().len()).ok()?;
        let start = span.end.checked_sub(length)?;
        self.references.get(&(span.file, start)).map(Option::as_ref)
    }
}

/// Framework declarations come from embedded Ruby implementations and
/// Ruby core RBS, never from fabricated suffix aliases. App sources keep
/// their paths as URIs so answers map back to their file IDs.
fn index_sources(sources: &[SourceFile], indexed_files: &[bool]) -> Graph {
    let mut documents = vec![Document {
        uri_prefix: CORE_URI,
        path: "",
        text: CORE_RBS,
        language: LanguageId::Rbs,
    }];
    documents.extend(crate::runtime_files::ruby_sources().map(|(path, text)| Document {
        uri_prefix: RUNTIME_URI_PREFIX,
        path,
        text,
        language: LanguageId::Ruby,
    }));
    documents.extend(sources.iter().zip(indexed_files).filter(|(_, ruby)| **ruby).map(
        |(source, _)| Document {
            uri_prefix: "",
            path: &source.path,
            text: &source.text,
            language: LanguageId::Ruby,
        },
    ));
    index_documents(&documents)
}

/// Rubydex resolves names, ownership, and lexical scope. Keep only its
/// immutable answers for positions Roundhouse will type; the full graph
/// leaves memory before the inference fixpoint starts.
fn collect_answers(graph: &Graph, sources: &[SourceFile], indexed_files: &[bool]) -> Answers {
    let mut references = HashMap::new();
    let mut class_cache: IdentityHashMap<DeclarationId, (Arc<ClassId>, bool)> =
        IdentityHashMap::default();
    for (index, source) in sources.iter().enumerate() {
        if !indexed_files[index] {
            continue;
        }
        let file = FileId((index + 1) as u32);
        let uri = UriId::from(source.path.as_str());
        let Some(document) = graph.documents().get(&uri) else {
            continue;
        };
        for reference_id in document.constant_references() {
            let Some(reference) = graph.constant_references().get(reference_id) else {
                continue;
            };
            let offset = reference.offset();
            // Rubydex also records a synthetic `<Article>` reference
            // at an `Article.method` receiver. Only the token's
            // written name belongs to this expression's source span.
            let written = source
                .text
                .get(offset.start() as usize..offset.end() as usize);
            let name = graph
                .names()
                .get(reference.name_id())
                .and_then(|name| graph.strings().get(name.str()));
            if !name.is_some_and(|name| written == Some(name.as_str())) {
                continue;
            }
            let target = graph
                .name_id_to_declaration_id(*reference.name_id())
                .and_then(|id| graph.declarations().get(id).map(|decl| (*id, decl)))
                .map(|(id, declaration)| {
                    if declaration.as_namespace().is_some()
                        && !is_assigned_value(graph, declaration)
                    {
                        let (class, runtime) = class_cache.entry(id).or_insert_with(|| {
                            (
                                Arc::new(ClassId(Symbol::from(declaration.name()))),
                                is_runtime_declaration(graph, declaration),
                            )
                        });
                        ResolvedConstant::Namespace {
                            class: Arc::clone(class),
                            runtime: *runtime,
                        }
                    } else {
                        ResolvedConstant::Value(id)
                    }
                });
            match references.entry((file, offset.start())) {
                Entry::Vacant(slot) => {
                    slot.insert(target);
                }
                Entry::Occupied(mut slot) => {
                    if slot.get().is_none() && target.is_some() {
                        slot.insert(target);
                    }
                }
            }
        }
    }
    references
}
