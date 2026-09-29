//! Points in time compare across their classes.
//!
//! ```ruby
//! def overdue?
//!   expires_at < CUTOFF_DATE || expires_at < DateTime.now
//! end
//! ```
//!
//! `Time`, `Date`, `DateTime` and `ActiveSupport::TimeWithZone` order against
//! one another (ActiveSupport teaches `Time#<=>` to take a `Date`), and only
//! the first was recognised as a time, so a cutoff of type `Time | Date`
//! against a `Time` was a mismatch. A `nil` arm is not temporal and a time is
//! still not comparable to a number.

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
            "ActiveRecord::Schema.define(version: 1) do\n  create_table \"things\", force: :cascade do |t|\n    t.datetime \"expires_at\", null: false\n    t.datetime \"seen_at\"\n  end\nend\n",
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
fn time_date_and_datetime_order_against_each_other() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  CUTOFF = Date.new(2020, 1, 1)
  def a
    expires_at < CUTOFF
  end
  def b
    expires_at > DateTime.now
  end
  def c(flag)
    cutoff = flag ? CUTOFF : Time.now
    expires_at > cutoff
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_nil_arm_is_still_not_temporal() {
    let found = binops(
        "class Thing < ApplicationRecord\n  def bad\n    seen_at > Time.now\n  end\nend\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
