//! `self.table_name = "remote_domains"` binds a model to that table.
//!
//! Shopify's `DomainSubscription` stores in `remote_domains`. Ingest
//! derived the conventional `domain_subscriptions`, found no such table
//! and gave the model NO columns, so `subscription.shop_id` (a column
//! of `remote_domains`) read as "no known method `shop_id` on
//! DomainSubscription?" -- 38 errors on core, and 783 models declare
//! their table this way.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(model_body: &str) -> Vec<String> {
    let files: Vec<(&str, String)> = vec![
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"remote_domains\", force: :cascade do |t|\n    t.integer \"shop_id\"\n  end\nend\n".into(),
        ),
        (
            "app/models/domain_subscription.rb",
            format!("class DomainSubscription < ApplicationRecord\n{model_body}\n  def self.owner\n    find_by(id: 1).shop_id\n  end\nend\n"),
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
fn a_renamed_table_supplies_the_models_columns() {
    let f = failures("  self.table_name = \"remote_domains\"");
    assert!(f.is_empty(), "unexpected dispatch failures: {f:?}");
}

#[test]
fn without_the_declaration_the_conventional_table_is_used() {
    let f = failures("");
    assert_eq!(f.len(), 1, "no domain_subscriptions table, so no shop_id: {f:?}");
}
