//! A plain class under `app/controllers/` is a class, not a controller.
//!
//! Shopify keeps small objects beside the controllers that use them: a
//! request-options struct nested in a concern
//! (`RequestStatsMiddlewareOptions::Options`), redirection strategies
//! (`Redirection::Strategies::Base`), a `FormBuilder` subclass. With no
//! `*Controller` class in the file, the first class was ingested as a
//! controller anyway. A controller's `initialize` is typed as an
//! action: keyword defaults are not folded into the parameter types and
//! no ivar environment is built for it, so the ivars it assigns
//! (`@enabled = enabled`) read `has no known type` in every other
//! method.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::diagnose;
use roundhouse::ingest::ingest_app_from_tree;

fn errors(files: &[(&str, &str)]) -> (Vec<String>, roundhouse::App) {
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest tree");
    roundhouse::session::analyze_and_lower(&mut app);
    let errors = diagnose(&app)
        .into_iter()
        .map(|d| d.to_string())
        .filter(|d| d.starts_with("error"))
        .collect();
    (errors, app)
}

const APPLICATION: (&str, &str) = (
    "app/controllers/application_controller.rb",
    "class ApplicationController < ActionController::Base\nend\n",
);

#[test]
fn a_struct_nested_in_a_concern_types_its_ivars_from_keyword_defaults() {
    let (o, app) = errors(&[
        APPLICATION,
        (
            "app/controllers/concerns/request_stats_options.rb",
            r#"
module RequestStatsOptions
  extend ActiveSupport::Concern

  class Options
    def initialize(enabled: false, only: nil)
      @enabled = enabled
      @only = Array(only).map(&:to_s) unless only.nil?
    end

    def enabled_for_action?(action)
      @enabled && (@only.nil? || @only.include?(action))
    end
  end
end
"#,
        ),
    ]);
    assert!(o.is_empty(), "{o:#?}");
    assert!(
        app.controllers.iter().all(|c| !c.name.0.as_str().ends_with("Options")),
        "`Options` is not a controller"
    );
    assert!(
        app.library_classes.iter().any(|c| c.name.0.as_str() == "RequestStatsOptions::Options"),
        "it is a library class"
    );
}

#[test]
fn a_form_builder_subclass_and_a_base_class_are_library_classes() {
    let (o, _) = errors(&[
        APPLICATION,
        (
            "app/controllers/auth/form_builder.rb",
            r#"
module Auth
  class AuthFormBuilder < ActionView::Helpers::FormBuilder
    def text_field(method, options = {})
      @template.content_tag(:label, options[:label], for: method)
    end
  end
end
"#,
        ),
        (
            "app/controllers/strategies/base.rb",
            r#"
module Strategies
  class Base
    def initialize(shop:, design_mode_options: {})
      @shop = shop
      @design_mode_options = design_mode_options
    end

    def options
      @design_mode_options[:in_design_mode]
    end
  end
end
"#,
        ),
    ]);
    assert!(o.is_empty(), "{o:#?}");
}

#[test]
fn a_class_descending_from_a_controller_is_still_a_controller() {
    let (_, app) = errors(&[
        APPLICATION,
        (
            "app/controllers/auth/section.rb",
            "module Auth\n  class Section < ApplicationController\n    def index; end\n  end\nend\n",
        ),
    ]);
    assert!(
        app.controllers.iter().any(|c| c.name.0.as_str() == "Auth::Section"),
        "a `< ApplicationController` class in app/controllers is a controller whatever its name"
    );
}
