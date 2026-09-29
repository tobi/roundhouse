//! `Array#intersect?` and `Hash#all?`: collection predicates the
//! catalog was missing.
//!
//! Shopify's models test `roles.intersect?(REQUIRED)` (Ruby 3.1's
//! non-allocating `(a & b).any?`) and validate a settings hash with
//! `flags.all? { |key, on| on }`. `Array` had `exclude?` and `Hash` had
//! `any?`/`none?` but neither of these, so each failed dispatch as
//! "no known method `intersect?` on Array[String]" / "`all?` on
//! Hash[String, bool]".
use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(body: &str) -> Vec<String> {
    let controller = format!("class ThingsController < ApplicationController\n{body}\nend\n");
    let files: Vec<(&str, &str)> = vec![
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        ("app/controllers/things_controller.rb", controller.as_str()),
        (
            "config/routes.rb",
            "Rails.application.routes.draw do\n  get \"/things\", to: \"things#index\"\nend\n",
        ),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    roundhouse::session::analyze_and_lower(&mut app);
    diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, recv_ty } => Some(format!(
                "{}#{}",
                roundhouse::ide::render_ty(&recv_ty),
                method.as_str()
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn an_array_intersects_and_a_hash_answers_all() {
    let f = failures(
        "  def index\n    a = %w[a b].intersect?(%w[b c])\n    b = { \"x\" => true }.all? { |key, on| on }\n    c = { \"x\" => 1 }.one?\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "predicate dispatch failures: {f:?}");
}
