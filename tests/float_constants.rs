//! `Float::INFINITY` is a number, not a class named `Float::INFINITY`.
//!
//! Range clamps and open-ended limits are written against it all over a Rails
//! app:
//!
//! ```ruby
//! def cap
//!   limit = position < Float::INFINITY ? position : Float::INFINITY
//!   Float::INFINITY - limit
//! end
//! ```
//!
//! The constant resolved to a class type, so `position < Float::INFINITY` was
//! an "Integer < Float::INFINITY" mismatch and `Float::INFINITY - x` had a
//! class for a left operand. The constants Ruby defines on `Float` are Floats.

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
fn float_constants_take_part_in_arithmetic_and_comparison() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def below?
    position.to_i < Float::INFINITY
  end
  def headroom
    Float::INFINITY - position.to_i
  end
  def scaled(width)
    width.to_f * Float::EPSILON
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_float_constant_is_still_not_a_string() {
    let found = binops(
        "class Thing < ApplicationRecord\n  def bad\n    \"a\" + Float::INFINITY\n  end\nend\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
