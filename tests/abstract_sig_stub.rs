//! A `sig { abstract… }` method in a concern is a declaration, not a body.
//!
//! Shopify's controller concerns state what they need from the includer:
//!
//! ```ruby
//! sig { abstract.returns(ActionController::Parameters) }
//! def params; end
//! ```
//!
//! The empty `def` was spliced into every including controller as a real
//! method answering `nil`, ahead of the `params` the controller inherits,
//! so `params.fetch_type(...)` failed with "no known method on nil" in
//! every class that included the concern (and in the concern itself).

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

#[test]
fn an_abstract_stub_does_not_shadow_the_includers_implementation() {
    let files: &[(&str, &str)] = &[
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        (
            "app/controllers/concerns/cm.rb",
            "module Cm\n  extend T::Helpers\n  abstract!\n\n  def go\n    params.permit(:a)\n  end\n\n  sig { abstract.returns(ActionController::Parameters) }\n  def params; end\nend\n",
        ),
        (
            "app/controllers/things_controller.rb",
            "class ThingsController < ApplicationController\n  include Cm\n  def index\n    params.fetch_type({ a: String }, nil)\n    render plain: \"x\"\n  end\nend\n",
        ),
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
    let failures: Vec<String> = diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, recv_ty } => Some(format!(
                "{}#{}",
                roundhouse::ide::render_ty(&recv_ty),
                method.as_str()
            )),
            _ => None,
        })
        .collect();
    assert!(failures.is_empty(), "dispatch failures: {failures:?}");
}
