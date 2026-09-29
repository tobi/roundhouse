//! core's `UrlHelpers` includes the route helpers through an untyped
//! `self`, one line down from a type-assertion comment. That is the same
//! `include` as the bare form, and the marker it records is what makes
//! `::UrlHelpers.<route>_url` resolve.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

#[test]
fn self_include_of_the_route_url_helpers_is_the_route_helper_surface() {
    // core's `UrlHelpers` includes the helpers through an untyped
    // `self`, one line down from a type-assertion comment. That is the
    // same `include` as the bare form, and the marker it records is what
    // makes `::UrlHelpers.<route>_url` resolve.
    let mut tree: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    tree.insert(
        PathBuf::from("app/models/url_helpers.rb"),
        br#"module UrlHelpers
  class << self
    self #: as untyped
      .include(Rails.application.routes.url_helpers)
  end
end
"#
        .to_vec(),
    );
    tree.insert(
        PathBuf::from("config/routes.rb"),
        b"Rails.application.routes.draw do\n  get \"ping\", to: \"ping#show\"\nend\n".to_vec(),
    );
    tree.insert(
        PathBuf::from("db/schema.rb"),
        b"ActiveRecord::Schema[7.1].define(version: 1) do\nend\n".to_vec(),
    );
    let app = ingest_app_from_tree(tree).expect("ingest tree");
    let helpers = app
        .library_classes
        .iter()
        .find(|c| c.name.0.as_str() == "UrlHelpers")
        .expect("UrlHelpers ingested");
    assert!(
        helpers.includes.iter().any(|i| i.0.as_str() == "RouteHelpers"),
        "{:?}",
        helpers.includes
    );
}
