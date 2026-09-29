//! Arithmetic between numbers of different classes is defined.
//!
//! A value that is `1.5` on one path and `2` on another is `Float | Integer`,
//! and Ruby adds, subtracts, multiplies, divides and takes the remainder of
//! any two numbers whatever their classes:
//!
//! ```ruby
//! def remaining(elapsed)
//!   budget = elapsed > 10 ? 1.5 : 2
//!   10.0 - budget
//! end
//! ```
//!
//! Only the pairs `Integer`/`Float` were modelled for these operators, while
//! comparison already accepted any union of numbers, so `Float - (Float |
//! Integer)` was reported as incompatible where `Float < (Float | Integer)`
//! was not. A `nil` arm is still a real mismatch.

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};

fn binops(model: &str) -> Vec<String> {
    let tree: std::collections::HashMap<std::path::PathBuf, Vec<u8>> = [
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\nend\n",
        ),
        ("app/models/thing.rb", model),
        ("db/schema.rb", "ActiveRecord::Schema.define(version: 1) do\nend\n"),
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
fn every_arithmetic_operator_accepts_a_float_or_integer_operand() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def all_of_them(flag)
    n = flag ? 1.5 : 2
    [10.0 - n, 10.0 + n, 10.0 * n, 10.0 / n, 10.0 % n, 10.0 ** n, n - 1, n * 2.0]
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_nil_arm_still_makes_the_arithmetic_a_mismatch() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def maybe(flag)
    n = flag ? 1.5 : nil
    10.0 - n
  end
end
"#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
