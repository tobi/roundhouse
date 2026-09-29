//! ActiveSupport's String conversions that a params scalar goes through.
//!
//! `params[:start_date].to_date`, `params[:percent].to_d / 100`,
//! `params[:at].to_time`, `params[:time]&.in_time_zone` and
//! `params[:capabilities].as_json` are how Shopify's controllers turn a
//! request string into a value. `String#to_date` and friends, `to_d` and
//! the `Object#as_json`/`to_json` pair were missing from the String
//! surface, so each failed dispatch on the very type the request value
//! is most often. Dates fold into `Time` and a decimal into `Float`, as
//! everywhere else in the analyzer.

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
fn a_string_parses_to_a_date_a_decimal_and_json() {
    let f = failures(
        "  def index\n    a = \"2020-01-01\".to_date\n    b = \"12:00\".to_time\n    c = \"1.5\".to_d / 100\n    d = \"now\".in_time_zone\n    e = \"x\".to_json\n    f = \"x\".as_json\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "string conversion dispatch failures: {f:?}");
}

#[test]
fn a_params_scalar_converts_through_them() {
    let f = failures(
        "  def index\n    a = params[:start_date]&.to_date\n    b = params[:percent].to_d / 100\n    c = params[:at].to_time\n    d = params[:time]&.in_time_zone\n    e = params[:tags].as_json\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "params conversion dispatch failures: {f:?}");
}

#[test]
fn a_collection_answers_as_json() {
    let f = failures(
        "  def index\n    a = [1, 2].as_json\n    b = { a: 1 }.as_json\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "as_json dispatch failures: {f:?}");
}
