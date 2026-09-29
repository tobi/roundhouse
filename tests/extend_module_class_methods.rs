//! `extend Mod` makes Mod's instance methods the class's own.
//!
//! Shopify core's `User` says `extend TrackCurrent`, and the module
//! defines `def current` / `def with_current` as ordinary instance
//! methods: on the extending class they are `User.current` and
//! `User.with_current`. Only `include` was folded into the registry, so
//! every such call (10 for `current` alone) was an unknown method.

use std::process::Command;

fn check(files: &[(&str, &str)]) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("rh_extend_mod_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, src) in files {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, src).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_roundhouse"))
        .args(["check", "--continue"])
        .arg(&dir)
        .output()
        .expect("spawn roundhouse");
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stderr).into_owned()
}

const MODULE: &str = "module TrackCurrent\n  def current\n    Thread.current.thread_variable_get(\"x\")\n  end\n\n  def with_current(user)\n    yield\n  end\nend\n";
const SCHEMA: &str = "ActiveRecord::Schema.define do\n  create_table \"users\" do |t|\n    t.string \"name\"\n  end\nend\n";
const CONTROLLER: &str = "class ApplicationController < ActionController::Base\n  def index\n    User.current\n    User.with_current(1) { 1 }\n    head :ok\n  end\nend\n";
const ROUTES: &str = "Rails.application.routes.draw do\n  root to: \"application#index\"\nend\n";

fn files<'a>(user: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("app/utils/track_current.rb", MODULE),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n"),
        ("app/models/user.rb", user),
        ("db/schema.rb", SCHEMA),
        ("app/controllers/application_controller.rb", CONTROLLER),
        ("config/routes.rb", ROUTES),
    ]
}

#[test]
fn an_extended_module_contributes_class_methods() {
    let err = check(&files("class User < ApplicationRecord\n  extend TrackCurrent\nend\n"));
    assert!(err.contains("0 error(s)"), "{err}");
}

/// Without the `extend`, the same calls really are undefined — the fold
/// must not invent class methods for a module nobody extended.
#[test]
fn an_unextended_module_contributes_nothing() {
    let err = check(&files("class User < ApplicationRecord\nend\n"));
    assert!(err.contains("no known method `current` on User"), "{err}");
}
