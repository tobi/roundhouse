//! Binary operators whose receiver is not a builtin value type.
//!
//! Core code does arithmetic on `Money`, `MultiCurrency::MoneyBag` and
//! `Gem::Version` (their classes define `+ - * <=>`), adds seconds to a
//! `Time` (`Time.now.utc + 10`), and subtracts arrays of unlike element
//! types (`[:a, "b"] - ["b"]`). All are valid Ruby; the classifiers used to
//! read every non-Int/Float/Str pair as a guaranteed TypeError.

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
fn class_owned_and_time_operators_are_not_type_errors() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  TTL = 10
  def expires = Time.now.utc + TTL
  def later = Time.now + 1.5
  def net(a, b)
    total = Money.new(0, "USD")
    total - Money.new(1, "USD")
  end
  def mixed(x)
    [x.to_s, :sym] - ["y"]
  end
  def score(score)
    (score || 5) <= 2.5
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn genuine_type_errors_are_still_reported() {
    let found = binops("class Thing < ApplicationRecord\n  def bad = 1 + \"x\"\nend\n");
    assert_eq!(found.len(), 1, "{found:?}");
}
