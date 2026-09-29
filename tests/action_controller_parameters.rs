//! `params` is an `ActionController::Parameters`, not a `Hash[Symbol, String]`.
//!
//! Shopify's controllers reach for the typed_parameters gem's extensions
//! on it (`params.fetch_type({ token: String }, nil)`, `require_type`,
//! `require_hash`, `permit_types`) and for the strong-parameters chain on
//! what `params[:order]` holds (`params[:order].permit(...)`). Typed as a
//! String-valued Hash, every one of them failed dispatch ("no known
//! method `require_type` on Hash[Symbol, String]"), and `params[:order]`
//! was a `String?` that could not be permitted. A `Parameters.new(...)`
//! local had no methods at all. The hash conversions on a permitted
//! `to_h` (`symbolize_keys`) were the same kind of gap on Hash itself.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(files: &[(&str, &str)]) -> Vec<String> {
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
fn parameters_answer_the_typed_parameters_and_strong_parameters_surface() {
    let f = failures(&[
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        (
            "app/controllers/things_controller.rb",
            "class ThingsController < ApplicationController\n  def index\n    a = params.require_type(mode: String)\n    b = params[:order].permit_types(line_items: [Hash])[:line_items]\n    c = params[:customer]&.permit_types(x: String)\n    d = params.permit(:fts).to_h.symbolize_keys\n    p = ActionController::Parameters.new(a: 1)\n    p[:b] = 2\n    p.merge(c: 1)\n    e = params.require_hash(:k)\n    g = params.fetch_type({ t: String }, nil)\n    h = params.permit(:x).to_unsafe_h\n    render plain: \"x\"\n  end\nend\n",
        ),
        (
            "config/routes.rb",
            "Rails.application.routes.draw do\n  get \"/things\", to: \"things#index\"\nend\n",
        ),
    ]);
    assert!(f.is_empty(), "params dispatch failures: {f:?}");
}
