//! RBS trailing types retain typing; non-nil assertions require executable semantics.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn failed_sends(body: &str) -> Vec<(String, String)> {
    let files: [(&str, &str); 5] = [
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"lines\", force: :cascade do |t|\n    t.integer \"qty\", null: false\n  end\nend\n",
        ),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        ("app/models/line.rb", "class Line < ApplicationRecord\n  def cents\n    1\n  end\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        ("app/controllers/cart_controller.rb", body),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    let mut out: Vec<(String, String)> = diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, recv_ty } => {
                Some((method.as_str().to_string(), format!("{recv_ty:?}")))
            }
            DiagnosticKind::Unsupported { construct, detail, .. } => Some((construct.to_string(), detail)),
            _ => None,
        })
        .collect();
    out.sort();
    out
}

/// The methods that failed to dispatch.
fn failures(body: &str) -> Vec<String> {
    failed_sends(body).into_iter().map(|(m, _)| m).collect()
}

/// The receiver type each failure names.
fn receivers(body: &str) -> Vec<String> {
    failed_sends(body).into_iter().map(|(_, r)| r).collect()
}

#[test]
fn a_cast_after_a_methods_last_expression_types_what_it_returns() {
    let f = failures(
        r#"class CartController < ApplicationController
  def line
    params[:x] #: as Line
  end

  def show
    line.cents.bogus_from_return
  end
end
"#,
    );
    assert_eq!(f, vec!["bogus_from_return".to_string()]);
}

#[test]
fn a_cast_after_an_argument_on_its_own_line_types_that_argument() {
    let f = failures(
        r#"class CartController < ApplicationController
  def show
    total(
      params[:x], #: as Line
    )
  end

  def total(line)
    line.cents.bogus_from_argument
  end
end
"#,
    );
    assert_eq!(f, vec!["bogus_from_argument".to_string()]);
}

#[test]
fn as_not_nil_is_a_named_refusal() {
    // A dispatch failure names the receiver's type. `Line.find_by` is
    // `Line | nil`; with `#: as !nil` it is a `Line`.
    let unchecked = receivers(
        r#"class CartController < ApplicationController
  def show
    line = Line.find_by(id: 1)
    line.bogus_after
  end
end
"#,
    );
    assert_eq!(unchecked.len(), 1, "{unchecked:?}");
    assert!(unchecked[0].contains("Nil"), "{unchecked:?}");
    let checked = receivers(
        r#"class CartController < ApplicationController
  def show
    line = Line.find_by(id: 1) #: as !nil
    line.bogus_after
  end
end
"#,
    );
    assert!(checked.iter().any(|d| d.contains("nil-check")), "{checked:?}");
    assert!(checked.iter().any(|d| d.contains("Nil")), "{checked:?}");
}

#[test]
fn a_comment_on_an_attribute_declaration_is_not_a_cast() {
    // `attr_reader :x #: Integer` documents the attribute. It must not
    // turn into a cast of the `attr_reader` call.
    let f = failures(
        r#"class CartController < ApplicationController
  attr_reader :count #: Integer

  def show
    count
  end
end
"#,
    );
    assert!(f.is_empty(), "{f:?}");
}
