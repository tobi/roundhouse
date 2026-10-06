//! A declared nominal type requires a real declaration or modeled dependency.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

#[test]
fn nonexistent_declared_classes_are_refused() {
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
        .filter(|d| matches!(d.kind, DiagnosticKind::Unsupported { ref construct, .. } if construct.as_str() == "declared type"))
        .collect();
    assert!(bad.iter().any(|d| d.message.contains("GemMoney")), "{:?}", bad.iter().map(|d| &d.kind).collect::<Vec<_>>());
}
