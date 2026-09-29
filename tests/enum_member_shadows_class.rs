//! A bare constant read finds a constant declared inside ANOTHER class
//! only through that class's own scope; from anywhere else a top-level
//! class of the same name is what Ruby answers.
//!
//! Shopify core's `GiftCard::SourceType < T::Enum` has a member named
//! `ApiClient`, and there is also a top-level `ApiClient` model. The
//! app-wide by-bare-name constant map held the enum member, and it won
//! over the class for every bare `ApiClient` in the app: 152
//! `ApiClient.find` / `.where` / `.find_by` calls failed dispatch
//! "on GiftCard::SourceType".

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
fn a_bare_class_read_is_not_captured_by_an_enum_member_of_the_same_name() {
    let out = failures(&[
        SCHEMA,
        APP_RECORD,
        APP_CONTROLLER,
        ROUTES,
        ("app/models/api_client.rb", "class ApiClient < ApplicationRecord\nend\n"),
        (
            "app/models/gift_card.rb",
            "class GiftCard\n  class SourceType < T::Enum\n    enums do\n      Purchased = new(\"purchased\")\n      ApiClient = new(\"api_client\")\n    end\n  end\nend\n",
        ),
        (
            "app/controllers/things_controller.rb",
            "class ThingsController < ApplicationController\n  def show\n    ApiClient.find(1)\n    ApiClient.find_by(api_key: \"k\")\n  end\nend\n",
        ),
    ]);
    assert!(out.is_empty(), "unexpected dispatch failures: {out:?}");
}
