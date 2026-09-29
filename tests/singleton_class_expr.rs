//! `class << self` at expression position.
//!
//! `included do … class << self … end … end` (a concern's hook block) is
//! ingested as an ordinary expression, where no class-body walk
//! classifies the block. It used to hit the "unsupported expression
//! node" fallthrough; it now evaluates to its last statement, with its
//! defs lifting to nil like any bare `def`.

use std::collections::HashMap;
use std::path::PathBuf;

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


/// At expression position — inside an `included do` block, where no
/// class-body walk classifies it — the block evaluates to its last
/// statement and its defs lift to nil like any bare `def`.
#[test]
fn a_singleton_block_at_expression_position_ingests() {
    let (_, gaps) = surveyed(&[(
        "app/controllers/concerns/tracked.rb",
        "module Tracked\n  extend ActiveSupport::Concern\n\n  included do\n    class << self\n      def tracked = true\n    end\n  end\nend\n",
    )]);
    assert!(gaps.is_empty(), "{gaps:?}");
}
