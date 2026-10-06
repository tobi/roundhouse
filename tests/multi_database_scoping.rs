//! Connection routing requires a runtime implementation, even on abstract records.

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
        .filter(|d| d.code() == "send_dispatch_failed" || d.code() == "unsupported")
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
fn connected_to_is_refused_without_routing_runtime() {
    let out = failures(&[
        SCHEMA,
        APP_RECORD,
        APP_CONTROLLER,
        ROUTES,
        ("app/models/api_client.rb", "class ApiClient < ApplicationRecord\nend\n"),
        (
            "app/models/core_general_2_record.rb",
            "class CoreGeneral2Record < ApplicationRecord\n  self.abstract_class = true\n  connects_to database: { writing: :a, reading: :b }\nend\n",
        ),
        (
            "app/controllers/things_controller.rb",
            "class ThingsController < ApplicationController\n  def show\n    CoreGeneral2Record.connected_to(role: :reading) do\n      ApiClient.find(1)\n    end\n  end\nend\n",
        ),
    ]);
    assert!(out.iter().any(|d| d.contains("connected_to")), "{out:?}");
}
