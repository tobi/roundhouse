//! Ruby constant references indexed from the application's Ruby sources.
//!
//! Rubydex resolves Ruby lexical names once, then releases its graph.
//! Roundhouse retains source-position answers and keys inferred values
//! by Rubydex declaration IDs.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;

use rubydex::indexing::{IndexerBackend, LanguageId, build_local_graph};
use rubydex::model::declaration::Declaration;
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

pub(crate) struct ConstResolver {
    references: HashMap<(FileId, u32), Option<ResolvedConstant>>,
    indexed_files: Vec<bool>,
}

/// Every URI is new in this analysis. Bulk merge avoids incremental
/// invalidation of each previously indexed document in a large app.
fn index_new_source(graph: &mut Graph, uri: String, text: &str, language: LanguageId) {
    graph.extend(build_local_graph(
        uri,
        text,
        &language,
        IndexerBackend::RubyIndexer,
    ));
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

impl ConstResolver {
    /// Index real Ruby app sources with their original 1-based file IDs.
    /// Framework declarations come from embedded Ruby implementations
    /// and Ruby core RBS, never from fabricated suffix aliases.
    pub(crate) fn from_app_sources(sources: &[SourceFile]) -> Self {
        let mut graph = Graph::new();
        index_new_source(&mut graph, CORE_URI.to_owned(), CORE_RBS, LanguageId::Rbs);
        for (path, text) in crate::runtime_files::ruby_sources() {
            index_new_source(
                &mut graph,
                format!("{RUNTIME_URI_PREFIX}{path}"),
                text,
                LanguageId::Ruby,
            );
        }

        let mut indexed_files = Vec::with_capacity(sources.len());
        for source in sources {
            let is_ruby = source.path.ends_with(".rb");
            indexed_files.push(is_ruby);
            if is_ruby {
                index_new_source(
                    &mut graph,
                    source.path.clone(),
                    &source.text,
                    LanguageId::Ruby,
                );
            }
        }
        // Rubydex resolves all source references after every file enters
        // its graph, including declarations in later files.
        Resolver::new(&mut graph).resolve();

        // Rubydex resolves names, ownership, and lexical scope. Keep only
        // its immutable answers for positions Roundhouse will type; the
        // full graph leaves memory before the inference fixpoint starts.
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
                        if declaration.as_namespace().is_some() {
                            let (class, runtime) = class_cache.entry(id).or_insert_with(|| {
                                (
                                    Arc::new(ClassId(Symbol::from(declaration.name()))),
                                    is_runtime_declaration(&graph, declaration),
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
