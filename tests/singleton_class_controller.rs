//! A controller's `class << self` and `def self.x`.
//!
//! ApplicationController's `class << self; def on_field_error … end` and
//! AdminSectionController's `dangerously_opt_out_of_cookie_protection_report_only`
//! are the whole class-side API of a controller. The controller ingester
//! had no class-side notion: `class << self` reached the expression
//! ingester, which refuses it, and the recorded gap made attribution
//! reclassify every unresolved thing in every inheriting controller and
//! view (26864 notes off application_controller.rb alone). `def self.x`
//! was worse: it was filed as an instance ACTION named `x`.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::diagnose;
use roundhouse::dialect::{ControllerBodyItem, MethodReceiver};
use roundhouse::ingest::{ingest_app_from_tree, survey};

const SCHEMA: &str = "ActiveRecord::Schema.define(version: 1) do\n  \
    create_table :gauges do |t|\n    t.string :label\n  end\nend\n";

fn tree(files: &[(&str, &str)]) -> HashMap<PathBuf, Vec<u8>> {
    let mut tree: HashMap<PathBuf, Vec<u8>> = [
        ("db/schema.rb", SCHEMA),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        (
            "config/routes.rb",
            "Rails.application.routes.draw do\n  get \"/gauges\", to: \"gauges#index\"\nend\n",
        ),
    ]
    .into_iter()
    .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
    .collect();
    for (p, c) in files {
        tree.insert(PathBuf::from(*p), c.as_bytes().to_vec());
    }
    tree
}

/// Survey-mode ingest, the way `roundhouse check --continue` runs it:
/// the app, and every gap recorded on the way.
fn surveyed(files: &[(&str, &str)]) -> (roundhouse::App, Vec<String>) {
    survey::activate();
    let app = ingest_app_from_tree(tree(files)).expect("ingest");
    let gaps = survey::drain()
        .iter()
        .map(|g| format!("{g:?}"))
        .collect();
    (app, gaps)
}


const BASE_CONTROLLER: &str = r#"class ApplicationController < ActionController::Base
  class << self
    def on_field_error(callback = nil, &block)
      @field_error_callback = block || callback
    end
  end

  def self.audited?
    true
  end
end
"#;

/// The controller half: no gap, and the defs are class-side methods of
/// the controller, not actions.
#[test]
fn a_controller_singleton_block_defines_class_methods_and_no_gap() {
    let (app, gaps) = surveyed(&[("app/controllers/application_controller.rb", BASE_CONTROLLER)]);
    assert!(gaps.is_empty(), "no `unsupported expression node`: {gaps:?}");
    let c = app
        .controllers
        .iter()
        .find(|c| c.name.0.as_str() == "ApplicationController")
        .expect("controller");
    let names: Vec<&str> = c.class_methods().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["on_field_error", "audited?"]);
    assert!(
        c.class_methods().all(|m| m.receiver == MethodReceiver::Class),
        "class-receiver methods"
    );
    assert_eq!(c.actions().count(), 0, "and not actions (nothing routes to them)");
    assert!(
        !c.body.iter().any(|i| matches!(i, ControllerBodyItem::Unknown { .. })),
        "nothing left over as unrecognized"
    );
}

/// A subclass reaches the class-side method through the registry, and a
/// method nobody defines is still reported (the negative twin: a gate
/// that only checks a clean ledger passes with the fix taken out).
#[test]
fn a_subclass_calls_the_inherited_class_method_and_a_typo_is_still_reported() {
    let sub = |call: &str| {
        format!(
            "class GaugesController < ApplicationController\n  on_field_error {{ 1 }}\n  def index\n    {call}\n    head :ok\n  end\nend\n"
        )
    };
    let errors = |call: &str| -> Vec<String> {
        let ctrl = sub(call);
        let mut app = ingest_app_from_tree(tree(&[
            ("app/controllers/application_controller.rb", BASE_CONTROLLER),
            ("app/controllers/gauges_controller.rb", ctrl.as_str()),
        ]))
        .expect("ingest");
        roundhouse::session::analyze_and_lower(&mut app);
        diagnose(&app)
            .into_iter()
            .map(|d| d.to_string())
            .filter(|d| d.starts_with("error"))
            .collect()
    };
    assert!(errors("self.class.audited?").is_empty(), "{:?}", errors("self.class.audited?"));
    assert!(
        errors("self.class.audted?").iter().any(|e| e.contains("audted?")),
        "a misspelled class method is still an error: {:?}",
        errors("self.class.audted?")
    );
}

