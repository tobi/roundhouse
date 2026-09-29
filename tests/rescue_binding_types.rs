//! `rescue A, B => e` and namespaced error classes.
//!
//! Core's authentication code rescues errors declared as
//! `class X < ApplicationError` deep inside modules, sometimes two at a
//! time (`rescue Foo::A, Foo::B => e`), and reads the domain readers
//! off `e`. Binding `e` as plain `StandardError` in those cases turned
//! every `e.metric_value` into a `send_dispatch_failed`.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

fn diagnostics_for(body: &str) -> Vec<String> {
    let files: Vec<(&str, String)> = vec![
        ("db/schema.rb", "ActiveRecord::Schema.define do\n  create_table \"users\", force: :cascade do |t|\n    t.string \"name\"\n  end\nend\n".into()),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n".into()),
        ("app/models/user.rb", "class User < ApplicationRecord\nend\n".into()),
        ("app/models/auth/application_error.rb", r#"module Auth
  class ApplicationError < StandardError
    #: () -> String
    attr_reader :metric_value
  end
end
"#.into()),
        ("app/models/auth/adapters/vendor.rb", r#"module Auth
  module Adapters
    class Vendor
      class TokenError < ApplicationError
        #: () -> Symbol
        def status; :ok; end
      end
      class OtherError < ApplicationError
        #: () -> Symbol
        def status; :bad; end
      end
    end
  end
end
"#.into()),
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n".into()),
        ("app/controllers/auth/users_controller.rb", format!("module Auth\n  class UsersController < ApplicationController\n    def index\n{body}\n    end\n\n    #: (Adapters::Vendor::TokenError) -> void\n    def typed(error)\n      error.metric_value\n      error.status\n    end\n  end\nend\n")),
        ("config/routes.rb", "Rails.application.routes.draw do\n  get \"/users\", to: \"auth/users#index\"\nend\n".into()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
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

fn dispatch_failures(diags: &[String]) -> Vec<&String> {
    diags.iter().filter(|d| d.contains("send_dispatch_failed")).collect()
}

#[test]
fn a_namespaced_error_class_binds_as_itself() {
    let diags = diagnostics_for("      begin\n        1\n      rescue Adapters::Vendor::TokenError => e\n        e.metric_value\n        e.status\n      end");
    assert!(dispatch_failures(&diags).is_empty(), "{diags:?}");
}

#[test]
fn a_multi_class_rescue_binds_as_the_union() {
    let diags = diagnostics_for("      begin\n        1\n      rescue Adapters::Vendor::TokenError, Adapters::Vendor::OtherError => e\n        e.metric_value\n        e.status\n      end");
    assert!(dispatch_failures(&diags).is_empty(), "{diags:?}");
}

#[test]
fn a_typed_parameter_of_a_namespaced_error_class_resolves() {
    let diags = diagnostics_for("      1");
    assert!(dispatch_failures(&diags).is_empty(), "{diags:?}");
}
