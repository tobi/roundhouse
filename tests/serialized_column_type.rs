//! `serialize :col, coder: JSON` makes the attribute read back as the
//! coder's object, not the column's storage text.
//!
//! Shopify's `Checkout` stores `line_items` in a text column and
//! declares `serialize :line_items, coder: …`; callers do
//! `checkout.line_items.map { … }`. The schema says `text`, so the
//! reader was typed `String?` and ~50 such calls on core read as
//! "no known method `map` on String?".

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(model_body: &str, use_site: &str) -> Vec<String> {
    let files: Vec<(&str, String)> = vec![
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"checkouts\", force: :cascade do |t|\n    t.text \"line_items\"\n  end\nend\n".into(),
        ),
        (
            "app/models/checkout.rb",
            format!("class Checkout < ApplicationRecord\n{model_body}\n  def total\n    {use_site}\n  end\nend\n"),
        ),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> =
        files.into_iter().map(|(p, c)| (PathBuf::from(p), c.into_bytes())).collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .iter()
        .filter(|d| d.code() == "send_dispatch_failed")
        .map(|d| format!("{d:?}"))
        .collect()
}

#[test]
fn a_serialized_column_is_not_typed_as_its_storage_text() {
    let f = failures("  serialize :line_items, coder: JSON", "line_items.map { |i| i }");
    assert!(f.is_empty(), "unexpected dispatch failures: {f:?}");
}

#[test]
fn an_unserialized_text_column_is_still_a_string() {
    let f = failures("", "line_items.map { |i| i }");
    assert_eq!(f.len(), 1, "String has no map: {f:?}");
}
