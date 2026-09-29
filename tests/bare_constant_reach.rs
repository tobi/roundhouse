//! A bare constant reaches a class the way Ruby reaches it: through the
//! lexical scope, then the ancestors, then the top level -- not by
//! sharing a last segment with some class buried in the app.
//!
//! The analyzer used to fall back to "the one registered class whose
//! name ends in `::Name`". For the framework catalog that is a fair
//! stand-in for top-level aliases (`HashWithIndifferentAccess`), but for
//! the app's own classes it captured a gem's top-level constant: with
//! `Apps::Listeners::Monorail` declared somewhere in the app, every bare
//! `Monorail.track(...)` (the gem's `Monorail`) was typed as that
//! unrelated class, and the calls failed against its methods.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer};
use roundhouse::ingest::ingest_app_from_tree;

fn diagnostics(extra: &[(&str, &str)], body: &str) -> Vec<String> {
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

const LISTENER: (&str, &str) = (
    "app/services/apps/listeners/monorail.rb",
    "module Apps\n  module Listeners\n    class Monorail\n      def self.publish; 1; end\n    end\n  end\nend\n",
);

#[test]
fn a_bare_name_is_not_captured_by_a_nested_app_class_of_that_name() {
    // The read is not inside `Apps::Listeners`, so `Monorail` is the top
    // level's, not that class -- and `track` is not its method either.
    let found = diagnostics(&[LISTENER], "    @a = Monorail.track");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("`Monorail`") || found[0].ends_with(" on Monorail"), "{found:?}");
    assert!(!found[0].contains("Apps::Listeners"), "{found:?}");
}

#[test]
fn a_sibling_in_the_lexical_scope_still_resolves() {
    let found = diagnostics(
        &[
            LISTENER,
            (
                "app/services/apps/listeners/relay.rb",
                "module Apps\n  module Listeners\n    class Relay\n      def go; Monorail.publish; end\n    end\n  end\nend\n",
            ),
        ],
        "    @a = Apps::Listeners::Relay.new.go",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_subclass_reads_a_class_nested_in_its_superclass() {
    let found = diagnostics(
        &[
            (
                "app/services/base_job.rb",
                "class BaseJob\n  class Helper\n    def self.help; 1; end\n  end\nend\n",
            ),
            (
                "app/services/sub_job.rb",
                "class SubJob < BaseJob\n  def go; Helper.help; end\nend\n",
            ),
        ],
        "    @a = SubJob.new.go",
    );
    assert!(found.is_empty(), "{found:?}");
}
