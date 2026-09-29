//! Everyday String methods the catalog was missing.
//!
//! `email.ascii_only?`, `body.count("\n")`, `value.exclude?("?")`,
//! `text.gsub!(/_handle$/, "")`, `token.first(4)`, `bytes.unpack("Q>")`
//! and `chars.each_char` are plain Ruby / ActiveSupport String surface;
//! each failed dispatch ("no known method `ascii_only?` on String") on
//! a value that is unambiguously a String.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, DiagnosticKind};
use roundhouse::ingest::ingest_app_from_tree;

fn failures(body: &str) -> Vec<String> {
    let controller = format!("class ThingsController < ApplicationController\n{body}\nend\n");
    let files: Vec<(&str, &str)> = vec![
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        ("app/controllers/things_controller.rb", controller.as_str()),
        (
            "config/routes.rb",
            "Rails.application.routes.draw do\n  get \"/things\", to: \"things#index\"\nend\n",
        ),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    roundhouse::session::analyze_and_lower(&mut app);
    diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, recv_ty } => Some(format!(
                "{}#{}",
                roundhouse::ide::render_ty(&recv_ty),
                method.as_str()
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn a_string_answers_its_everyday_surface() {
    let f = failures(
        "  def index\n    s = \"abc\"\n    a = s.ascii_only?\n    b = s.count(\"a\")\n    c = s.exclude?(\"?\")\n    d = s.gsub!(/b/, \"\")\n    e = s.first(2)\n    f = s.last(2)\n    g = s.insert(1, \"-\")\n    h = s.to_str\n    i = s.unpack(\"Q>\")\n    j = s.unpack1(\"C\")\n    k = s.slice!(0..1)\n    s.each_char { |ch| ch }\n    s.encode(\"UTF-8\")\n    render plain: \"x\"\n  end",
    );
    assert!(f.is_empty(), "string dispatch failures: {f:?}");
}
