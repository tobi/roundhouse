//! A class that defines `method_missing` answers every message the rest
//! of its ancestor chain does not. A send it catches is not a dispatch
//! failure, and a real method still wins over it.
//!
//! core's `CustomerAccountUrlHelpers` (`customer_account_*_url` becomes a
//! `UrlHelpers` send at runtime) and `UrlHelpers` itself (its class-side
//! `include Rails.application.routes.url_helpers`) are the two shapes.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn dispatch_failures(files: &[(&str, &str)]) -> Vec<String> {
    let mut tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(*p), c.as_bytes().to_vec()))
        .collect();
    tree.insert(PathBuf::from("db/schema.rb"), b"ActiveRecord::Schema.define do\nend\n".to_vec());
    tree.insert(
        PathBuf::from("app/controllers/application_controller.rb"),
        b"class ApplicationController < ActionController::Base\nend\n".to_vec(),
    );
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, .. } => Some(method.as_str().to_string()),
            _ => None,
        })
        .collect()
}

const DELEGATOR: (&str, &str) = (
    "app/models/delegator.rb",
    r#"module Delegator
  class << self
    def method_missing(method_name, *args, &block)
      return unless method_name.to_s.start_with?("wrapped_")

      args.first
    end

    def respond_to_missing?(method_name, _)
      method_name.to_s.start_with?("wrapped_")
    end

    def real(x)
      x
    end
  end
end
"#,
);

const CALLER: (&str, &str) = (
    "app/controllers/foo_controller.rb",
    r#"class FooController < ApplicationController
  def show
    @url = Delegator.wrapped_thing_url({ host: "x" })
    @real = Delegator.real(1)
    render plain: @url.to_s
  end
end
"#,
);

#[test]
fn a_message_only_method_missing_answers_is_not_a_dispatch_failure() {
    let failed = dispatch_failures(&[DELEGATOR, CALLER]);
    assert!(failed.is_empty(), "{failed:?}");
}

#[test]
fn without_method_missing_the_same_send_still_fails() {
    let plain = (
        "app/models/delegator.rb",
        "module Delegator\n  class << self\n    def real(x)\n      x\n    end\n  end\nend\n",
    );
    let failed = dispatch_failures(&[plain, CALLER]);
    assert_eq!(failed, vec!["wrapped_thing_url"]);
}
