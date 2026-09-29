//! A parameter's declared type versus what call sites were seen to pass.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn dispatch_failures(files: &[(&str, &str)]) -> Vec<String> {
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    let mut out: Vec<String> = diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, .. } => Some(method.as_str().to_string()),
            _ => None,
        })
        .collect();
    out.sort();
    out
}

const SCHEMA: (&str, &str) = ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n");

/// The declared type of a parameter wins over what one caller was seen
/// passing.
///
/// `login_with_shop_controller.rb` has `#: (FatalError error) -> void`
/// above a helper, and one caller hands it a value the analysis knows
/// only as `nil`. Observation used to win, so `error` was `nil` inside
/// the helper and `error.message` failed with "message on nil" even
/// though the signature says exactly what `error` is.
#[test]
fn a_fully_declared_parameter_keeps_its_type_when_a_caller_passes_nil() {
    let failures = dispatch_failures(&[
        SCHEMA,
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        (
            "app/controllers/x_controller.rb",
            r#"class XController < ApplicationController
  class FatalError < StandardError
    def key
      :k
    end
  end

  def show
    render_fatal_error(nil)
  end

  #: (FatalError error) -> void
  def render_fatal_error(error)
    error.key.bogus_from_key
  end
end
"#,
        ),
    ]);
    assert_eq!(failures, vec!["bogus_from_key".to_string()]);
}
