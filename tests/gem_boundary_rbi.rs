//! A locked gem's Tapioca RBI is a typed boundary to the gem.
//!
//! `sorbet/rbi/gems/<gem>@<version>.rbi` declares the gem's classes and
//! method headers. Reading it means a call into a gem class answers
//! with what the RBI says, instead of the whole gem surface being
//! unknown. The tests pin the three properties that make that safe:
//! a declared method is typed, an undeclared one on a fully declared
//! class is a real miss, and everything that is NOT fully declared
//! (unresolved names, `method_missing`, an app subclass) stays gradual.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

const LOCK: &str = "GEM\n  remote: https://rubygems.org/\n  specs:\n    zork (1.0)\n    activesupport (8.0.2)\n\nDEPENDENCIES\n  zork\n  activesupport\n";

const ZORK: &str = r#"# typed: true
module Zork
  class Widget
    sig { returns(::String) }
    def label; end

    sig { params(n: ::Integer).returns(::Zork::Widget) }
    def self.build(n); end

    sig { returns(::Elsewhere::Thing) }
    def thing; end

    def bare(a, b = 1); end

    sig { void }
    def poke; end

    attr_reader :size
  end

  class Ghost
    def method_missing(name, *args); end
  end

  class Mixed
    include ::Unmodelled::Mixin
  end
end
"#;

/// Diagnostics for `body` run inside an action of an app locked to the
/// fake gem `zork` (whose RBI is `rbi`), as `code message` lines.
fn diagnostics(rbi: &str, extra: &[(&str, &str)], body: &str) -> Vec<String> {
    let mut files: Vec<(String, String)> = vec![
        (
            "db/schema.rb".into(),
            "ActiveRecord::Schema.define do\n  create_table \"things\", force: :cascade do |t|\n    t.string \"name\"\n  end\nend\n".into(),
        ),
        (
            "app/models/application_record.rb".into(),
            "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n".into(),
        ),
        (
            "app/controllers/application_controller.rb".into(),
            "class ApplicationController < ActionController::Base\nend\n".into(),
        ),
        (
            "config/routes.rb".into(),
            "Rails.application.routes.draw do\n  get \"/things\", to: \"things#show\"\nend\n".into(),
        ),
        ("Gemfile.lock".into(), LOCK.into()),
        ("sorbet/rbi/gems/zork@1.0.rbi".into(), rbi.into()),
        (
            "app/controllers/things_controller.rb".into(),
            format!("class ThingsController < ApplicationController\n  def show\n{body}\n  end\nend\n"),
        ),
    ];
    files.extend(extra.iter().map(|(p, c)| (p.to_string(), c.to_string())));
    let tree: HashMap<PathBuf, Vec<u8>> =
        files.into_iter().map(|(p, c)| (PathBuf::from(p), c.into_bytes())).collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .iter()
        .filter(|d| d.code() == "send_dispatch_failed")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn a_declared_method_answers_its_signature() {
    // `build` -> Widget, `label` -> String: `upcase` exists on String.
    let found = diagnostics(ZORK, &[], "    @a = Zork::Widget.build(1).label.upcase");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_result_typed_by_the_rbi_is_checked_like_any_typed_value() {
    let found = diagnostics(ZORK, &[], "    @a = Zork::Widget.build(1).label.no_such_string_method");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("no_such_string_method") && found[0].contains("String"), "{found:?}");
}

#[test]
fn an_undeclared_method_on_a_fully_declared_class_is_a_real_miss() {
    let found = diagnostics(ZORK, &[], "    @a = Zork::Widget.build(1).no_such_method");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("no_such_method") && found[0].contains("Zork::Widget"), "{found:?}");
}

#[test]
fn a_class_name_that_resolves_to_nothing_is_untyped_not_a_phantom_class() {
    // `Elsewhere::Thing` is in no registry. Typing `thing` as that class
    // would make `.anything` a dispatch failure on a class nobody defines.
    let found = diagnostics(ZORK, &[], "    @a = Zork::Widget.build(1).thing.anything");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn unsigned_void_and_attr_members_exist_and_answer_untyped() {
    let body = "    w = Zork::Widget.build(1)\n    @a = w.bare(1).x\n    @b = w.poke.y\n    @c = w.size.z";
    let found = diagnostics(ZORK, &[], body);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_class_with_method_missing_is_open() {
    let found = diagnostics(ZORK, &[], "    @a = Zork::Ghost.new.whatever");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_class_including_an_unmodelled_mixin_is_open() {
    let found = diagnostics(ZORK, &[], "    @a = Zork::Mixed.new.from_the_mixin");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn an_app_subclass_keeps_the_gradual_escape_for_inherited_macros() {
    // The gem's class-level macros generate instance methods on the app
    // subclass, and no declaration lists them.
    let found = diagnostics(
        ZORK,
        &[("app/models/panel.rb", "class Panel < Zork::Widget\nend\n")],
        "    @a = Panel.new.generated_by_a_macro\n    @b = Panel.new.label.upcase",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_class_the_app_defines_is_not_overridden() {
    let found = diagnostics(
        ZORK,
        &[("app/models/zork/widget.rb", "module Zork\n  class Widget\n    def own; 1; end\n  end\nend\n")],
        "    @a = Zork::Widget.new.own",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn rails_component_rbis_are_not_read() {
    // activesupport is the analyzer's own catalog; its RBI would be a
    // second, partial, description of it.
    let app = {
        let tree: HashMap<PathBuf, Vec<u8>> = [
            ("Gemfile.lock", LOCK),
            ("sorbet/rbi/gems/activesupport@8.0.2.rbi", "module Shadow\n  class Only\n  end\nend\n"),
            ("sorbet/rbi/gems/zork@1.0.rbi", ZORK),
        ]
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
        ingest_app_from_tree(tree).expect("ingest")
    };
    let has = |name: &str| {
        app.gem_boundary.classes.contains_key(&roundhouse::ident::ClassId(roundhouse::ident::Symbol::new(name)))
    };
    assert!(has("Zork::Widget"));
    assert!(!has("Shadow::Only"));
}
