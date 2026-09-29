//! A guard on an attribute of `self` speaks for the reads it guards.
//!
//! Rails models are full of nullable columns compared right after the check
//! that makes the comparison safe:
//!
//! ```ruby
//! def expired?
//!   expires_at.present? && expires_at <= Time.now.utc
//! end
//! def overlaps?(other)
//!   start_at.present? && end_at.present? && start_at >= end_at
//! end
//! ```
//!
//! `expires_at` here is `self.expires_at`: a column reader, no different from a
//! local for the length of the condition. The nil arm of `Time?` has to go on
//! the guarded side, or every such guard is reported as `Time? <= Time`.
//!
//! Two things make that sound rather than a blanket "trust the guard": only a
//! receiver-less, zero-argument read of a declared attribute (a column,
//! `attr_accessor`) is narrowed, so `find(1)`-like calls and methods that
//! merely happen to be nilable are still reported, and a compound condition
//! only claims what it proves (`a && b` false says nothing about either).

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
    t.datetime "expires_at"
    t.datetime "start_at"
    t.datetime "end_at"
    t.integer "position"
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
fn a_guarded_column_reader_is_not_nilable_in_the_guarded_read() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def expired?
    expires_at.present? && expires_at <= Time.now.utc
  end
  def live?
    expires_at.nil? || expires_at > Time.now.utc
  end
  def positive?
    !position.nil? && position > 0
  end
  def in_order?
    return false if start_at.blank? || end_at.blank?
    start_at < end_at
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn every_conjunct_of_a_guard_narrows() {
    let found = binops(
        "class Thing < ApplicationRecord\n  def a\n    start_at.present? && end_at.present? && start_at >= end_at\n  end\nend\n",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn an_unguarded_or_wrongly_guarded_column_is_still_reported() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def a
    expires_at <= Time.now.utc
  end
  def b
    expires_at.present? || expires_at > Time.now.utc
  end
  def c
    expires_at.present? && start_at < Time.now.utc
  end
  def d
    return false if expires_at.nil? && start_at.nil?
    expires_at > Time.now.utc
  end
end
"#,
    );
    assert_eq!(found.len(), 4, "{found:?}");
}
