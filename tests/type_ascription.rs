//! `T.let(x, Type)` and the RBS inline `x = value #: Type` say what a
//! value is; ingest used to keep the value and drop the words. Shopify
//! core's `AuthorizeController` does
//! `@preview_token = T.let(result.ok_value, T.nilable(Tokens::ProviderPreviewToken))`
//! where `result.ok_value` is beyond the analyzer, so every later read of
//! `@preview_token` reported `ivar_unresolved` although the source states
//! the type outright.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn unresolved_ivars(controller: &str) -> Vec<String> {
    let files = [
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n"),
        ("app/controllers/foo_controller.rb", controller),
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
fn t_let_types_an_ivar_whose_value_is_unknown() {
    let names = unresolved_ivars(
        r#"class FooController < ApplicationController
  def show
    @token = T.let(Mystery.build.ok_value, T.nilable(String))
    render plain: @token.to_s
  end
end
"#,
    );
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn a_trailing_rbs_type_types_an_assignment() {
    let names = unresolved_ivars(
        r#"class FooController < ApplicationController
  def show
    @raw = mystery #: ActionController::Parameters
    @id ||= mystery2 #: Integer?
    render plain: @raw.to_s + @id.to_s
  end
end
"#,
    );
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn without_an_annotation_the_ivar_stays_unresolved() {
    let names = unresolved_ivars(
        r#"class FooController < ApplicationController
  def show
    @raw = mystery
    render plain: @raw.to_s
  end
end
"#,
    );
    assert_eq!(names, vec!["raw".to_string()]);
}
