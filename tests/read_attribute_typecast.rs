//! `self[:column]` reads the attribute typecast, like its own reader.
//!
//! Models reach for the bracket form to bypass an overridden reader:
//!
//! ```ruby
//! def next_position
//!   (self[:position] || 0) + 1
//! end
//! ```
//!
//! The catalog answers `[]` and `read_attribute` with `String`, the shape of
//! the storage text the lowered model deals in, so that was `String + Integer`.
//! The program reads the typecast value: an `Integer` for an integer column,
//! a `Time` for a datetime. A literal name that is an attribute is answered
//! with the column's type (nil-able like the reader); any other key keeps the
//! catalog's answer.

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};

fn binops(model: &str) -> Vec<String> {
    let tree: std::collections::HashMap<std::path::PathBuf, Vec<u8>> = [
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\nend\n",
        ),
        ("app/models/thing.rb", model),
        (
            "db/schema.rb",
            r#"ActiveRecord::Schema.define(version: 1) do
  create_table "things", force: :cascade do |t|
    t.integer "position"
    t.datetime "expires_at"
  end
end
"#,
        ),
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
fn bracket_and_read_attribute_answer_the_columns_type() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def next_position
    (self[:position] || 0) + 1
  end
  def previous_position
    (read_attribute(:position) || 0) - 1
  end
  def expired?
    (self[:expires_at] || Time.now) < Time.now
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_bracket_read_of_a_nilable_column_is_still_nilable() {
    let found = binops(
        "class Thing < ApplicationRecord\n  def bad\n    self[:position] + 1\n  end\nend\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
