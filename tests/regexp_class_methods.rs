//! `Regexp.escape`, `Regexp.last_match` and `Regexp.union` — the class
//! side that string-building code reaches for. The registry carried only
//! instance methods, so each was `no known method on Regexp`.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

#[test]
fn regexp_class_methods_dispatch() {
    let src = "class Finder\n  def call(s)\n    Regexp.escape(s).upcase\n    Regexp.quote(s)\n    Regexp.union(\"a\", \"b\") =~ s\n    Regexp.last_match\n  end\nend\n";
    let tree: HashMap<PathBuf, Vec<u8>> =
        [(PathBuf::from("app/models/finder.rb"), src.as_bytes().to_vec())].into_iter().collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    let residue = roundhouse::session::analyze_and_lower(&mut app);
    let diags: Vec<String> = residue
        .iter()
        .chain(roundhouse::analyze::diagnose(&app).iter())
        .map(roundhouse::diagnostic::Diagnostic::to_string)
        .filter(|d| d.contains("send_dispatch_failed"))
        .collect();
    assert!(diags.is_empty(), "{diags:?}");
}
