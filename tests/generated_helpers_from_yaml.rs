//! A helper object built at load time from a YAML table has methods no
//! `def` declares: core's `WebUrlHelpers` gets `<key>_path` / `<key>_url`
//! for every entry of `config/web_paths.yml`, because its factory
//! `define_method`s them. The set is the table's keys crossed with the
//! factory's name suffixes, so a call to a key the table has resolves
//! and a call to one it lacks is still a dispatch failure.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{Analyzer, DiagnosticKind, diagnose};
use roundhouse::ingest::ingest_app_from_tree;

const FACTORY: &str = r##"class PathsFactory
  class << self
    def create(paths)
      Class.new do
        paths.each do |config_name, path|
          define_method "#{config_name}_path" do |**args|
            path
          end

          define_method(
            "#{config_name}_url"
          ) do |host: nil, **args|
            path
          end
        end
      end.new
    end
  end
end
"##;

fn app(controller_body: &str) -> roundhouse::App {
    let files = [
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n".to_string()),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n".to_string(),
        ),
        ("app/models/paths_factory.rb", FACTORY.to_string()),
        (
            "app/models/web_paths.rb",
            "WebPaths = PathsFactory.create(YAML.load_file(File.join(Rails.root, \"config/web_paths.yml\"))[\"paths\"])\n"
                .to_string(),
        ),
        (
            "config/web_paths.yml",
            "# comment\npaths:\n  admin_orders: /admin/orders\n  admin_order_show: /admin/orders/:id\n".to_string(),
        ),
        (
            "app/controllers/foo_controller.rb",
            format!(
                "class FooController < ApplicationController\n  def show\n{controller_body}\n    render plain: \"\"\n  end\nend\n"
            ),
        ),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    ingest_app_from_tree(tree).expect("ingest")
}

fn failed_sends(mut app: roundhouse::App) -> Vec<String> {
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, .. } => Some(method.as_str().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn table_keys_crossed_with_factory_suffixes_are_the_method_set() {
    let app = app("");
    let names: Vec<&str> = app
        .generated_helper_methods
        .values()
        .flatten()
        .map(|n| n.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["admin_order_show_path", "admin_order_show_url", "admin_orders_path", "admin_orders_url"]
    );
}

#[test]
fn calls_the_table_defines_resolve() {
    let failed = failed_sends(app(
        "    @a = WebPaths.admin_orders_path\n    @b = WebPaths.admin_order_show_url(id: 1, host: \"x\")",
    ));
    assert!(failed.is_empty(), "{failed:?}");
}

#[test]
fn a_call_the_table_lacks_still_fails() {
    let failed = failed_sends(app("    @a = WebPaths.admin_nothing_path"));
    assert_eq!(failed, vec!["admin_nothing_path"]);
}
