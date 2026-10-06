//! Benchmark cannot be admitted when emitted projects lack its runtime dependency.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

fn diagnostics_for(action: &str) -> Vec<String> {
    let controller = format!(
        "class ReportsController < ApplicationController\n  def show\n    {action}\n  end\nend\n"
    );
    let tree: HashMap<PathBuf, Vec<u8>> = [
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"reports\", force: :cascade do |t|\n    t.string \"name\", null: false\n  end\nend\n".to_string(),
        ),
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\nend\n".to_string(),
        ),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n".to_string(),
        ),
        ("app/controllers/reports_controller.rb", controller),
        (
            "config/routes.rb",
            "Rails.application.routes.draw do\n  get \"/reports\", to: \"reports#show\"\nend\n".to_string(),
        ),
    ]
    .into_iter()
    .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
    .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    let residue = roundhouse::session::analyze_and_lower(&mut app);
    residue
        .iter()
        .chain(roundhouse::analyze::diagnose(&app).iter())
        .map(roundhouse::diagnostic::Diagnostic::to_string)
        .collect()
}


#[test]
fn benchmark_timing_helpers_are_refused() {
    for action in ["@a = Benchmark.realtime { 1 }.round(2)", "@a = Benchmark.ms { 1 }.round(1)"] {
        let diags = diagnostics_for(action);
        assert!(diags.iter().any(|d| d.contains("Benchmark")), "{action}: {diags:?}");
    }
}
