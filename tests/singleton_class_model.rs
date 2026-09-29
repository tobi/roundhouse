//! `class << self … end` in a model's or controller's body.
//!
//! Ruby runs the block with `self` set to the class's singleton class,
//! so a `def` defines a class method, `attr_accessor` a class-level
//! accessor, `include Mod` extends the class with `Mod`, and a constant
//! assignment lands in the block's own lexical scope.
//!
//! Two things went wrong on Shopify core, both by refusal:
//!
//! * A CONTROLLER's `class << self` (ApplicationController's
//!   `on_field_error`, AdminSectionController's
//!   `dangerously_opt_out_of_cookie_protection_report_only`) reached the
//!   expression ingester, which has no arm for it. The gap is recorded
//!   against the file, and gap attribution then reclassifies every
//!   unresolved thing in every controller and view that inherits from
//!   it as "likely roundhouse coverage": 28060 notes off two files.
//! * A MODEL's block refused at its first statement it did not know
//!   (`RATE = 5`, `sig { … }`, `include Mod`, `attr_accessor :x`) and
//!   dropped the whole block with it, so Shop, Order and Product lost
//!   their entire class-side API and every call into it was a
//!   `send_dispatch_failed`.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::dialect::{MethodReceiver, ModelBodyItem};
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

fn model_methods(app: &roundhouse::App, model: &str) -> Vec<(String, MethodReceiver)> {
    let m = app.models.iter().find(|m| m.name.0.as_str() == model).expect("model");
    m.body
        .iter()
        .filter_map(|item| match item {
            ModelBodyItem::Method { method, .. } => Some((method.name.to_string(), method.receiver)),
            _ => None,
        })
        .collect()
}

const GAUGE: &str = r#"class Gauge < ApplicationRecord
  module Metering
    def reading = 42
  end

  class << self
    RATE = 5

    include Metering

    attr_accessor :cache

    sig { returns(Integer) }
    def rate = RATE

    private

    def hidden = 2

    private def sneaky = 3

    alias_method :speed, :rate
    private_constant :RATE
  end

  def use = self.class.rate + Gauge.hidden
end
"#;

/// The whole block survives: every kind of statement in it is read, so
/// the defs beside a constant, a `sig`, an `include` and an accessor are
/// still there.
#[test]
fn a_model_singleton_block_keeps_its_methods_beside_the_statements_it_reads() {
    let (app, gaps) = surveyed(&[("app/models/gauge.rb", GAUGE)]);
    assert!(gaps.is_empty(), "nothing in the block is a gap: {gaps:?}");
    let methods = model_methods(&app, "Gauge");
    for name in ["rate", "hidden", "sneaky", "speed", "cache", "cache="] {
        assert!(
            methods.contains(&(name.to_string(), MethodReceiver::Class)),
            "`{name}` is a class method; got {methods:?}"
        );
    }
    // The constant is read by bare name from the block's own method, so
    // it has to be a constant of the class.
    let gauge = app.models.iter().find(|m| m.name.0.as_str() == "Gauge").unwrap();
    assert!(
        gauge.body.iter().any(|i| matches!(i, ModelBodyItem::Unknown { expr, .. }
            if format!("{:?}", expr.node).contains("RATE"))),
        "RATE is kept as a class-body constant"
    );
}

/// `include Mod` in the block extends the CLASS with `Mod`, and the
/// analyzer already folds a class-body `extend Mod`, so that is what the
/// block is replayed as.
#[test]
fn an_include_in_a_singleton_block_extends_the_class() {
    let (app, _) = surveyed(&[("app/models/gauge.rb", GAUGE)]);
    let gauge = app.models.iter().find(|m| m.name.0.as_str() == "Gauge").unwrap();
    assert!(
        gauge.body.iter().any(|i| matches!(i, ModelBodyItem::Unknown { expr, .. }
            if matches!(&*expr.node, roundhouse::expr::ExprNode::Send { recv: None, method, .. }
                if method.as_str() == "extend"))),
        "`include Metering` in `class << self` is `extend Metering`"
    );
}

/// A statement the walk cannot read costs itself: the gap names the call
/// (not just "CallNode"), and the block's defs survive it.
#[test]
fn an_unreadable_singleton_statement_is_named_and_does_not_drop_the_defs() {
    let (app, gaps) = surveyed(&[(
        "app/models/gauge.rb",
        "class Gauge < ApplicationRecord\n  class << self\n    memoize_all :rate\n    def rate = 1\n  end\nend\n",
    )]);
    assert_eq!(gaps.len(), 1, "one gap: {gaps:?}");
    assert!(gaps[0].contains("call to `memoize_all`"), "named: {gaps:?}");
    assert!(
        model_methods(&app, "Gauge").contains(&("rate".to_string(), MethodReceiver::Class)),
        "the def beside the unreadable call is kept"
    );
}

/// Strict ingest still refuses it: recovery is survey mode's.
#[test]
fn strict_ingest_still_refuses_an_unreadable_singleton_statement() {
    let err = ingest_app_from_tree(tree(&[(
        "app/models/gauge.rb",
        "class Gauge < ApplicationRecord\n  class << self\n    memoize_all :rate\n  end\nend\n",
    )]));
    assert!(err.is_err(), "a real gap is not silently dropped in strict mode");
}


/// `attr_accessor :cache` in the block synthesizes a writer whose body
/// reads its `value` parameter. Nothing in the source is at that node, so
/// a "local read `value` has unresolved type" warning could not be found
/// or fixed by the author: the residual-unresolved check skips nodes with
/// a synthetic span.
#[test]
fn a_synthesized_accessor_body_reports_no_unresolved_warning() {
    let mut app = ingest_app_from_tree(tree(&[(
        "app/models/gauge.rb",
        "class Gauge < ApplicationRecord\n  class << self\n    attr_accessor :cache\n  end\nend\n",
    )]))
    .expect("ingest");
    roundhouse::session::analyze_and_lower(&mut app);
    let noise: Vec<String> = roundhouse::analyze::diagnose(&app)
        .into_iter()
        .map(|d| d.to_string())
        .filter(|d| d.contains("`value`"))
        .collect();
    assert!(noise.is_empty(), "{noise:?}");
}
