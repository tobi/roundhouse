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

fn unresolved_ivars(provider: &str) -> Vec<String> {
    let files = [
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/controllers/cust/prov.rb", provider),
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
