//! `[x, LIMIT].min` / `.max` on an array literal.
//!
//! The clamp idiom (`limit = [limit, MAX].min`) is everywhere in
//! pagination code. A literal has at least one element, so the extremum is
//! an element, never the nil `Enumerable#min` gives an empty collection;
//! typing it `T?` made `MAX_OFFSET - restrict_limit(l)` look like
//! arithmetic on nil (`Integer?? - Integer`).

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};

fn binops(model: &str) -> Vec<String> {
    let tree: std::collections::HashMap<std::path::PathBuf, Vec<u8>> = [
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base
end
"),
        ("app/models/thing.rb", model),
    ]
    .iter()
    .map(|(p, c)| (std::path::PathBuf::from(p), c.as_bytes().to_vec()))
    .collect();
    let mut app = roundhouse::ingest::ingest_app_from_tree(tree).expect("ingest tree");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .into_iter()
        .filter(|d| matches!(d.kind, DiagnosticKind::IncompatibleBinop { .. }))
        .map(|d| format!("{:?}", d.kind))
        .collect()
}

#[test]
fn clamp_idiom_is_not_nilable() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def clamp(limit)
    limit = [limit, 5].min
    [limit, 1].max
  end
  def offset = 100 - clamp(3)
  def direct = 100 - [3, 4].min
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}
