//! `x.not_nil!` strips the nil arm of a nilable receiver.
//!
//! Core's style guide replaces Sorbet's `T.must(x)` with `x.not_nil!`
//! (Sorbet's `BasicObject#not_nil!`, defined on every object). The
//! method table has no entry for it on `String`/`Model` etc., and union
//! dispatch skips the nil arm, so `find_by(...).not_nil!` read as
//! "no known method `not_nil!` on User?" -- 42 such errors on Shopify
//! core. The answer is the receiver minus nil, so the chain continues
//! to type-check against the real class.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(controller_body: &str) -> Vec<String> {
    let files: Vec<(&str, String)> = vec![
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"users\", force: :cascade do |t|\n    t.string \"name\", null: false\n  end\nend\n".into(),
        ),
        ("app/models/user.rb", "class User < ApplicationRecord\nend\n".into()),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n".into(),
        ),
        (
            "app/controllers/users_controller.rb",
            format!("class UsersController < ApplicationController\n{controller_body}\nend\n"),
        ),
        (
            "config/routes.rb",
            "Rails.application.routes.draw do\n  resources :users\nend\n".into(),
        ),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> =
        files.into_iter().map(|(p, c)| (PathBuf::from(p), c.into_bytes())).collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .iter()
        .filter(|d| d.code() == "send_dispatch_failed")
        .map(|d| format!("{d:?}"))
        .collect()
}

#[test]
fn not_nil_lets_a_chain_continue_on_the_non_nil_class() {
    let f = failures("  def show\n    name = User.find_by(name: \"a\").not_nil!.name\n    name.upcase\n  end");
    assert!(f.is_empty(), "unexpected dispatch failures: {f:?}");
}

#[test]
fn not_nil_still_checks_the_stripped_type() {
    let f = failures("  def show\n    User.find_by(name: \"a\").not_nil!.frobnicate\n  end");
    assert_eq!(f.len(), 1, "frobnicate must still be reported once: {f:?}");
}

/// A receiver whose non-nil arm is itself un-inferred (`Var`) stays
/// gradual, the way union dispatch treats a `Var` arm, rather than
/// reporting `not_nil!` on `untyped?` (9 such on core).
#[test]
fn not_nil_on_an_uninferred_arm_is_not_an_error() {
    let f = failures("  def show\n    x = UnknownThing.lookup(1) || nil\n    x.not_nil!.anything\n  end");
    assert!(f.iter().all(|m| !m.contains("not_nil")), "not_nil! must not fail: {f:?}");
}
