//! A receiver whose declared class is an unmodeled gem class
//! (`Money`, `CSV::Row`, `ActionController::Parameters`) has no method
//! table to consult, so a send on it is a gradual escape, not
//! `send_dispatch_failed`. Declared types (T.let / RBS) surface such
//! receivers where before the value was simply unknown.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

#[test]
fn sends_on_an_unregistered_declared_class_are_not_failures() {
    let ctrl = r#"class FooController < ApplicationController
  def show
    @price = T.let(params[:x], T.nilable(GemMoney))
    render plain: @price.format.to_s
  end
end
"#;
    let files = [
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n"),
        ("app/controllers/foo_controller.rb", ctrl),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    let bad: Vec<_> = diagnose(&app)
        .into_iter()
        .filter(|d| matches!(d.kind, DiagnosticKind::SendDispatchFailed { .. }))
        .collect();
    assert!(bad.is_empty(), "{:?}", bad.iter().map(|d| &d.kind).collect::<Vec<_>>());
}
