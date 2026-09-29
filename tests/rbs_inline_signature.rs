//! RBS inline method signatures: `#: (::Shop, ActionDispatch::Request) -> void`
//! above a `def`. Shopify core's context providers declare their
//! constructor this way and then keep the arguments in ivars
//! (`@shop = shop`); the analyzer read only Sorbet `sig` blocks, so the
//! parameters were untyped and every read of the ivar reported
//! `ivar_unresolved`.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

/// The provider is written as a controller subclass: `diagnose` walks
/// controllers, models and views, so it is the shape whose ivar reads are
/// observable here. (A plain class under `app/controllers/` is a library
/// class, which is typed but not walked for diagnostics.)
fn unresolved_ivars(provider: &str) -> Vec<String> {
    let provider = provider.replace("class Prov\n", "class Prov < ApplicationController\n");
    let files = [
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        ("app/controllers/cust/prov.rb", provider.as_str()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::IvarUnresolved { name } => Some(name.as_str().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_comment_signature_types_the_constructor_arguments() {
    let names = unresolved_ivars(
        r#"module Cust
  class Prov
    # @override
    #: (::Shop, ActionDispatch::Request, ?limit: Integer) -> void
    def initialize(shop, request, limit: 1)
      @shop = shop
      @request = request
      @limit = limit
    end

    def go
      [@shop, @request, @limit]
    end
  end
end
"#,
    );
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn a_continued_signature_reads_as_one() {
    let names = unresolved_ivars(
        r#"module Cust
  class Prov
    #: (
    #|   String,
    #|   Integer
    #| ) -> void
    def initialize(a, b)
      @a = a
      @b = b
    end

    def go
      [@a, @b]
    end
  end
end
"#,
    );
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn a_signature_that_does_not_pair_with_the_def_is_dropped() {
    let names = unresolved_ivars(
        r#"module Cust
  class Prov
    #: (String, Integer) -> void
    def initialize(a)
      @a = a
    end

    def go
      @a
    end
  end
end
"#,
    );
    assert_eq!(names, vec!["a".to_string()]);
}

/// `Capabilities::Charge` written inside `ShopifyPayments::Capability`
/// names `ShopifyPayments::Capabilities::Charge`, and a bare `Hash` is a
/// hash you can index. Both were read as classes with nothing to call.
fn dispatch_failures(files: &[(&str, &str)]) -> Vec<String> {
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
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
fn a_signature_class_resolves_lexically_and_a_bare_hash_is_indexable() {
    let failures = dispatch_failures(&[
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        (
            "app/models/shopify_payments/capabilities/charge.rb",
            "module ShopifyPayments\n  module Capabilities\n    class Charge\n      def shop_id\n        1\n      end\n    end\n  end\nend\n",
        ),
        (
            "app/models/shopify_payments/capability.rb",
            r#"module ShopifyPayments
  class Capability
    #: (Array[Capabilities::Charge], Hash params) -> void
    def preload(charges, params)
      charges.each { |c| c.shop_id }
      params[:x]
    end
  end
end
"#,
        ),
    ]);
    assert!(failures.is_empty(), "{failures:?}");
}
