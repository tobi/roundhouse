//! A `Shopify::Adt::Result[Ok, Err]` signature reads as that class.
//!
//! Core names the parameterization once, `Result = T.type_alias {
//! Shopify::Adt::Result[Tokens, Error] }`, and every operation
//! returns `Result`. The sig reader only knew `T::Array` / `T::Hash`
//! as generic containers, so the alias body was unreadable, the alias
//! was dropped, and `-> Result` became a class literally named
//! `Result` — ~90 calls of `.ok_value` / `.error?` / `.ok?` failed
//! "on Result". The gem's surface is cataloged (catalog/gems.rs).

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(files: &[(&str, &str)]) -> Vec<String> {
    let tree: HashMap<PathBuf, Vec<u8>> =
        files.iter().map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec())).collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .iter()
        .filter(|d| d.code() == "send_dispatch_failed")
        .map(|d| d.message.clone())
        .collect()
}

const SCHEMA: (&str, &str) = (
    "db/schema.rb",
    "ActiveRecord::Schema.define do\n  create_table \"api_clients\", force: :cascade do |t|\n    t.string \"api_key\"\n  end\nend\n",
);
const APP_RECORD: (&str, &str) = (
    "app/models/application_record.rb",
    "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n",
);
const APP_CONTROLLER: (&str, &str) = (
    "app/controllers/application_controller.rb",
    "class ApplicationController < ActionController::Base\nend\n",
);
const ROUTES: (&str, &str) = (
    "config/routes.rb",
    "Rails.application.routes.draw do\n  get \"/things\", to: \"things#show\"\nend\n",
);

#[test]
fn a_generic_result_alias_answers_the_result_surface() {
    let out = failures(&[
        SCHEMA,
        APP_RECORD,
        APP_CONTROLLER,
        ROUTES,
        (
            "app/services/grant.rb",
            "module Tokens\n  class OpenIdTokens; end\nend\nclass Grant\n  Result = T.type_alias do\n    Shopify::Adt::Result[Tokens::OpenIdTokens,\n      T.any(String, Integer)]\n  end\n\n  sig { returns(Result) }\n  def self.perform\n    Shopify::Adt::Result.ok(Tokens::OpenIdTokens.new)\n  end\nend\n",
        ),
        (
            "app/controllers/things_controller.rb",
            "class ThingsController < ApplicationController\n  def show\n    r = Grant.perform\n    r.ok_value\n    r.error_value if r.error?\n    r.ok?\n  end\nend\n",
        ),
    ]);
    assert!(out.is_empty(), "unexpected dispatch failures: {out:?}");
}
