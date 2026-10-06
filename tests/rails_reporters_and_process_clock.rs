//! Registry-only reporters and undefined application tracers are not executable support.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

fn diagnostics_for(body: &str) -> Vec<String> {
    let controller = format!(
        "class XController < ApplicationController\n  def index\n{body}\n    head :ok\n  end\nend\n"
    );
    let files: [(&str, &str); 4] = [
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n"),
        ("app/controllers/x_controller.rb", &controller),
        ("config/routes.rb", "Rails.application.routes.draw do\n  get \"/x\", to: \"x#index\"\nend\n"),
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
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
fn unimplemented_reporters_and_tracers_are_refused() {
    let diags = diagnostics_for(
        r#"    Rails.event.notify("thing", message: "m")
    Rails.event.tagged(a: 1) { Rails.event.debug("d", message: "x") }
    Rails.error.report(StandardError.new("x"), handled: true)
    Rails.error.handle(StandardError) { 1 }
    Rails.error.set_context(a: 1)
    t = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    ms = Process.clock_gettime(Process::CLOCK_MONOTONIC, :millisecond)
    fm = Process.clock_gettime(Process::CLOCK_MONOTONIC, :float_millisecond)
    ShopifyTracer.in_span("x") { |s| 1 }
    [t + 1.0, ms + 1, fm + 1.0]"#,
    );
    for name in ["event", "error", "ShopifyTracer", "Process"] {
        assert!(diags.iter().any(|d| d.contains(name)), "missing {name}: {diags:?}");
    }
}
