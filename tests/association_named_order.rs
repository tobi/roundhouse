//! `belongs_to :order` on a model: `self.order` is the association
//! reader, not `ActiveRecord::QueryMethods#order`. Refund, Return and
//! several other core models belong to an Order, and reading it through
//! `T.must(self.order)` was typed as a `Relation[Refund]`, so every
//! following `order.line_items` / `order.id` was a send_dispatch_failed.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

#[test]
fn a_belongs_to_named_order_reads_as_the_association() {
    let files: Vec<(&str, &str)> = vec![
        ("db/schema.rb", "ActiveRecord::Schema.define do\n  create_table \"orders\", force: :cascade do |t|\n    t.string \"name\"\n  end\n  create_table \"refunds\", force: :cascade do |t|\n    t.integer \"order_id\"\n  end\nend\n"),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        ("app/models/order.rb", "class Order < ApplicationRecord\n  has_many :refunds\nend\n"),
        ("app/models/refund.rb", "class Refund < ApplicationRecord\n  belongs_to :order\n\n  def go\n    o = self.order\n    o.name\n    p = order\n    p.name\n  end\nend\n"),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    let residue = roundhouse::session::analyze_and_lower(&mut app);
    let diags: Vec<String> = residue
        .iter()
        .chain(roundhouse::analyze::diagnose(&app).iter())
        .map(roundhouse::diagnostic::Diagnostic::to_string)
        .filter(|d| d.contains("send_dispatch_failed"))
        .collect();
    assert!(diags.is_empty(), "{diags:?}");
}
