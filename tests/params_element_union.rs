//! A value read out of `params` is `String | Array | Parameters | nil`,
//! not the `String?` every scalar read wants.
//!
//! Shopify's controllers treat the same `params[:key]` as whatever the
//! request carried: `params[:pod_ids].map(&:to_i)` (an Array),
//! `params[:capabilities].as_json` and `params[:asset][:key] = ...` (a
//! nested Parameters), `params.require(:api_type).to_sym` (the scalar
//! under a required key), while `params[:id].to_i` and
//! `params[:start_date].to_date` read it as a String. Typed `String?`,
//! every non-scalar use failed dispatch ("no known method `map` on
//! String?"), and a value handed to a helper (`remove(params[:ids])`)
//! carried the wrong type into the helper's own body. Typed as the union,
//! a send resolves on whichever arms answer it, and an `is_a?` guard
//! selects the arm.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(controller_body: &str) -> Vec<String> {
    let controller = format!(
        "class ThingsController < ApplicationController\n{controller_body}\nend\n"
    );
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
fn an_element_read_answers_the_array_and_parameters_surface() {
    let f = failures(
        "  def index\n    a = params[:pod_ids].map(&:to_i)\n    b = params[:capabilities].as_json\n    params[:asset][:key] = \"k\"\n    c = params[:filters].to_unsafe_h\n    d = params[:tracking]&.fetch(:company, nil)\n    e = params[:app_ids].count\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "element read dispatch failures: {f:?}");
}

#[test]
fn an_element_read_still_answers_the_scalar_surface() {
    let f = failures(
        "  def index\n    a = params[:id].to_i\n    d = params[:name].to_s.strip.downcase\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "scalar dispatch failures: {f:?}");
}

#[test]
fn require_answers_the_value_under_the_key() {
    let f = failures(
        "  def index\n    a = params.require(:api_type).to_sym\n    b = params.require(:days).to_i\n    c = params.require(:item).permit(:name)\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "require dispatch failures: {f:?}");
}

#[test]
fn an_is_a_guard_selects_the_arm_of_the_element() {
    let f = failures(
        "  def index\n    statuses = params[:statuses]\n    return head(:bad_request) unless statuses.is_a?(Array) && statuses.present?\n    statuses.map { |s| s.to_s }\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "guarded dispatch failures: {f:?}");
}

#[test]
fn an_element_handed_to_a_helper_keeps_the_union_in_the_helper() {
    let f = failures(
        "  def index\n    remove(params[:address_ids])\n    render plain: \"x\"\n  end\n\n  private\n\n  def remove(ids)\n    ids.map { |id| id.to_i }\n  end",
    );
    assert!(f.is_empty(), "helper dispatch failures: {f:?}");
}

#[test]
fn parameters_read_as_a_hash_of_param_values() {
    let f = failures(
        "  def index\n    a = params.fetch(:queues, []).map(&:to_s)\n    b = params[:obj].values.map(&:to_s)\n    params[:obj].each { |k, v| v.to_s }\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "hash reading dispatch failures: {f:?}");
}
