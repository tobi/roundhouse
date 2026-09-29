//! A concern's `sig { abstract… }` methods are contracts, not bodies.
//!
//! Core's controller concerns declare what the includer must provide:
//!
//! ```ruby
//! module Privacy::CmpCookies
//!   extend T::Helpers
//!   abstract!
//!
//!   sig { abstract.returns(T.untyped) }
//!   def request; end
//!
//!   sig { abstract.returns(ActionController::Parameters) }
//!   def params; end
//! end
//! ```
//!
//! The empty body reads as `nil`. Splicing that copy into the includer
//! shadowed the framework's `request` / `params` with `nil`, so every
//! `request.host` / `params[:id]` in the controller (and in the concern
//! itself) failed with "no known method on nil" - ~1000 errors on core.
//! An abstract method is not copied into the includer, and a `sig` on a
//! non-abstract concern method also describes the spliced copy.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(concern: &str) -> Vec<String> {
    let files: [(&str, &str); 3] = [
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n"),
        ("app/controllers/concerns/cmp.rb", concern),
        (
            "app/controllers/things_controller.rb",
            "class ThingsController < ApplicationController\n  include Cmp\n  def index\n    request.host\n    request.headers[\"X\"]\n    params[:a]\n    bar.host\n  end\nend\n",
        ),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .iter()
        .filter(|d| d.code() == "send_dispatch_failed")
        .map(|d| format!("{d:?}"))
        .collect()
}

const CONCERN: &str = r#"module Cmp
  extend T::Helpers
  abstract!

  sig { abstract.returns(T.untyped) }
  def request; end

  sig { abstract.returns(ActionController::Parameters) }
  def params; end

  sig { returns(T.untyped) }
  def bar; nil; end

  def go
    request.host
    params[:x]
    bar.host
  end
end
"#;

#[test]
fn abstract_declarations_do_not_shadow_the_framework_in_the_includer() {
    let failed = failures(CONCERN);
    assert!(failed.is_empty(), "unexpected dispatch failures: {failed:?}");
}
