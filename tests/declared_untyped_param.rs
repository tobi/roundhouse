//! A parameter a signature declares `untyped` is not inferred from call sites.
//!
//! ```ruby
//! #: (untyped, untyped) -> bool
//! def compare(left, right)
//!   left > right
//! end
//! ```
//!
//! is handed Integers here, Strings there, and `nil` somewhere else. An
//! unannotated parameter is an inference gap the call sites may fill, but
//! `untyped` is the author saying the body must accept anything: unifying the
//! observed argument types into `Integer | String | nil` invents a
//! precondition the signature never made, and `left > right` was reported as
//! `Integer | String | nil > ...`. The unannotated neighbour keeps its
//! inferred type.

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};

fn errors(model: &str) -> Vec<String> {
    let tree: std::collections::HashMap<std::path::PathBuf, Vec<u8>> = [
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\nend\n",
        ),
        ("app/models/thing.rb", model),
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define(version: 1) do\n  create_table \"things\", force: :cascade do |t|\n    t.string \"name\"\n  end\nend\n",
        ),
    ]
    .iter()
    .map(|(p, c)| (std::path::PathBuf::from(p), c.as_bytes().to_vec()))
    .collect();
    let mut app = roundhouse::ingest::ingest_app_from_tree(tree).expect("ingest tree");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .into_iter()
        .filter(|d| {
            matches!(
                d.kind,
                DiagnosticKind::IncompatibleBinop { .. } | DiagnosticKind::SendDispatchFailed { .. }
            )
        })
        .map(|d| format!("{:?}", d.kind))
        .collect()
}

#[test]
fn an_untyped_parameter_accepts_whatever_the_call_sites_pass() {
    let found = errors(
        r#"class Thing < ApplicationRecord
  #: (untyped, untyped) -> bool
  def compare(left, right)
    left > right
  end

  def go
    compare(1, 2)
    compare("a", "b")
    compare(nil, 3)
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn an_unannotated_parameter_still_takes_the_type_its_callers_pass() {
    let found = errors(
        r#"class Thing < ApplicationRecord
  def plain(left)
    left + 1
  end

  def go
    plain("a")
  end
end
"#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
