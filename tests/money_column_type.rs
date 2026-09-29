//! The money gems' column DSLs (`money_column`, `money_bags`) retype an
//! attribute from its stored number to a value object.
//!
//! Shopify's `Checkout` stores `total_price` as a decimal and declares
//! `money_bags(attributes: [:total_price, ...])`; callers do
//! `checkout.total_price.shop_money`. The decimal column read as
//! `Float?`, so ~35 such calls reported "no known method `shop_money`
//! on Float?". `Money`/`MoneyBag` are gem classes roundhouse does not
//! model, so the honest type is gradual.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(model_body: &str) -> Vec<String> {
    let files: Vec<(&str, String)> = vec![
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"checkouts\", force: :cascade do |t|\n    t.decimal \"total_price\"\n    t.decimal \"amount\"\n  end\nend\n".into(),
        ),
        (
            "app/models/checkout.rb",
            format!("class Checkout < ApplicationRecord\n{model_body}\n  def self.shop\n    [find_by(id: 1).total_price.shop_money, find_by(id: 1).amount.currency]\n  end\nend\n"),
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
fn money_declarations_make_the_attribute_a_value_object() {
    let f = failures("  money_bags(attributes: [:total_price], money_bag_packer: proc { 1 })\n  money_column :amount, currency_column: :currency");
    assert!(f.is_empty(), "unexpected dispatch failures: {f:?}");
}

#[test]
fn undeclared_decimal_columns_are_still_numbers() {
    let f = failures("");
    assert_eq!(f.len(), 2, "a bare decimal has no shop_money or currency: {f:?}");
}
