//! A mixin an app adds to a framework class (`ActionDispatch::Request`)
//! is part of that class's surface: `request.buyer_session!` resolves
//! through `include(RequestBuyerSession)`, whether the include sits in a
//! plain reopen or inside the `ActiveSupport.on_load` block the app
//! defers it with.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{Analyzer, DiagnosticKind, diagnose};
use roundhouse::ingest::ingest_app_from_tree;

fn failed_sends(files: &[(&str, &str)]) -> Vec<String> {
    let mut tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(*p), c.as_bytes().to_vec()))
        .collect();
    tree.insert(PathBuf::from("db/schema.rb"), b"ActiveRecord::Schema.define do\nend\n".to_vec());
    tree.insert(
        PathBuf::from("app/controllers/application_controller.rb"),
        b"class ApplicationController < ActionController::Base\nend\n".to_vec(),
    );
    tree.insert(
        PathBuf::from("app/controllers/foo_controller.rb"),
        br#"class FooController < ApplicationController
  def show
    @s = request.buyer_session!
    @u = request.http_basic_user
    @ok = request.authorized?
    @n = request.nothing_defined
    render plain: ""
  end
end
"#
        .to_vec(),
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

#[test]
fn a_module_included_in_a_reopen_of_the_framework_class_is_its_surface() {
    let failed = failed_sends(&[(
        "lib/patches/request_buyer_session.rb",
        r#"module RequestBuyerSession
  #: -> String
  def buyer_session!
    "x"
  end
end

module ActionDispatch
  class Request
    include(RequestBuyerSession)
  end
end
"#,
    )]);
    assert_eq!(failed, vec!["http_basic_user", "authorized?", "nothing_defined"]);
}

#[test]
fn a_module_defined_and_included_inside_an_on_load_block_is_too() {
    let failed = failed_sends(&[(
        "lib/patches/http_authenticate.rb",
        r#"ActiveSupport.on_load(:action_dispatch_request) do
  module HttpAuthenticatePatch
    def authorized?
      true
    end

    def http_basic_user
      "u"
    end
  end

  include(HttpAuthenticatePatch)
end
"#,
    )]);
    assert_eq!(failed, vec!["buyer_session!", "nothing_defined"]);
}

#[test]
fn every_reopen_of_the_framework_class_contributes_its_mixins() {
    // core reopens `ActionDispatch::Request` in several patch files; the
    // registry used to keep only the last file's `include`.
    let failed = failed_sends(&[
        (
            "lib/patches/a.rb",
            "module ModA\n  def buyer_session!\n    1\n  end\nend\nmodule ActionDispatch\n  class Request\n    include(ModA)\n  end\nend\n",
        ),
        (
            "lib/patches/b.rb",
            "module ModB\n  def authorized?\n    true\n  end\nend\nmodule ActionDispatch\n  class Request\n    include(ModB)\n  end\nend\n",
        ),
    ]);
    assert_eq!(failed, vec!["http_basic_user", "nothing_defined"]);
}
