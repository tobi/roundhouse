//! Native Date-only values are distinct from timestamps. Mixed Date/Time
//! ordering is refused until the emitted runtime implements ActiveSupport's
//! cross-type coercion. Nil remains incompatible with timestamp ordering.

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
fn date_only_values_do_not_silently_take_timestamp_ordering() {
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
    assert_eq!(found.len(), 2, "Date and a Date/Time union must both refuse: {found:?}");
    assert!(found.iter().all(|d| d.contains("Date")), "{found:?}");
}

#[test]
fn a_nil_arm_is_still_not_temporal() {
    let found = binops(
        "class Thing < ApplicationRecord\n  def bad\n    seen_at > Time.now\n  end\nend\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
