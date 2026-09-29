//! A union of classes answers its operators arm by arm.
//!
//! ```ruby
//! def total(flag)
//!   amount = flag ? Money.new(5) : Money::Zero.new
//!   amount + amount
//! end
//! ```
//!
//! `+` on an instance of an app class is that class's own method, which the
//! binop classifier cannot see, so an operand that is one is left to
//! dispatch. A value that is one of several such classes was treated as a
//! mismatch instead of the same thing arm by arm; only a union with an arm
//! that owns no operator (`nil`) is a real mismatch.

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};

fn binops(files: &[(&str, &str)]) -> Vec<String> {
    let mut all: Vec<(&str, &str)> = vec![
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        ("db/schema.rb", "ActiveRecord::Schema.define(version: 1) do\nend\n"),
    ];
    all.extend_from_slice(files);
    let tree: std::collections::HashMap<std::path::PathBuf, Vec<u8>> = all
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

const MONEY: (&str, &str) = (
    "app/models/money.rb",
    "class Money\n  include ActiveModel::Model\n  def +(other)\n    self\n  end\n  def <=>(other)\n    0\n  end\nend\n",
);
const ZERO: (&str, &str) = (
    "app/models/money_zero.rb",
    "class MoneyZero\n  include ActiveModel::Model\n  def +(other)\n    other\n  end\n  def <=>(other)\n    0\n  end\nend\n",
);

#[test]
fn a_union_of_operator_owning_classes_is_left_to_dispatch() {
    let found = binops(&[
        MONEY,
        ZERO,
        (
            "app/models/thing.rb",
            r#"class Thing < ApplicationRecord
  def total(flag)
    amount = flag ? Money.new : MoneyZero.new
    amount + amount
  end
end
"#,
        ),
    ]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_nil_arm_in_that_union_is_still_a_mismatch() {
    let found = binops(&[
        MONEY,
        (
            "app/models/thing.rb",
            r#"class Thing < ApplicationRecord
  def total(flag)
    amount = flag ? Money.new : nil
    amount + amount
  end
end
"#,
        ),
    ]);
    assert_eq!(found.len(), 1, "{found:?}");
}
