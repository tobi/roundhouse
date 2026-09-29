//! Route DSL that composes routes without being a route itself. Each
//! shape used to drop whole subtrees, and with them every helper the
//! subtree defined (`shop_*_url`, `services_internal_*_path`, ...):
//!
//! - an app's own block-taking DSL methods, mixed into the mapper with
//!   `ActionDispatch::Routing::Mapper.prepend(Mod)` (`routing_method :x
//!   do ... end`), were "unsupported" and dropped their block;
//! - a second `Rails.application.routes.draw` in one file replaced the
//!   first instead of appending, and an `Engine.routes.draw` contributed
//!   its routes to the APP's route set;
//! - `load(...)` / `instance_eval(File.read(...))` of a `config/routes/`
//!   file was unsupported, and nested files collided on their stem;
//! - `concern` / `concerns` were unsupported;
//! - `only: []` meant "all seven actions" instead of none;
//! - `get :x, controller: "c"` had no controller.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;
use roundhouse::lower::routes::flatten_routes;

fn flat(files: &[(&str, &str)]) -> Vec<(String, String, String)> {
    let mut tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    tree.entry(PathBuf::from("db/schema.rb"))
        .or_insert_with(|| b"ActiveRecord::Schema[7.1].define(version: 1) do\nend\n".to_vec());
    let app = ingest_app_from_tree(tree).expect("ingest tree");
    flatten_routes(&app)
        .into_iter()
        .map(|r| (format!("{:?}", r.method).to_uppercase(), r.path.clone(), r.as_name.clone()))
        .collect()
}

fn paths(files: &[(&str, &str)]) -> Vec<String> {
    flat(files).into_iter().map(|(m, p, _)| format!("{m} {p}")).collect()
}

const MAPPER_PATCH: (&str, &str) = (
    "lib/patches/mapper.rb",
    "ActionDispatch::Routing::Mapper.prepend(Annotations)\n",
);

const ANNOTATIONS: (&str, &str) = (
    "lib/annotations.rb",
    r#"module Annotations
  def routing_method(method, &block)
    annotate({ routing_method: method }, &block)
  end

  def request_priority(priority, &block)
    annotate({ request_priority: priority }, &block)
  end

  def plain(x)
    x
  end
end
"#,
);

#[test]
fn prepended_mapper_block_methods_scope_their_block_without_changing_paths() {
    let got = paths(&[
        MAPPER_PATCH,
        ANNOTATIONS,
        (
            "config/routes.rb",
            r#"Rails.application.routes.draw do
  routing_method :main_pod do
    request_priority :high do
      get "ping", to: "ping#show"
    end
    namespace :services do
      resources :widgets, only: [:index]
    end
  end
end
"#,
        ),
    ]);
    assert_eq!(got, vec!["GET /ping", "GET /services/widgets"]);
}

#[test]
fn a_block_taking_method_the_app_never_mixed_in_is_not_a_scope() {
    // Without the `Mapper.prepend`, `routing_method` is an unknown
    // call: it is reported, and its block is not silently entered.
    let mut tree: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    tree.insert(PathBuf::from("lib/annotations.rb"), ANNOTATIONS.1.as_bytes().to_vec());
    tree.insert(
        PathBuf::from("config/routes.rb"),
        b"Rails.application.routes.draw do\n  routing_method :main_pod do\n    get \"ping\", to: \"ping#show\"\n  end\nend\n".to_vec(),
    );
    tree.insert(
        PathBuf::from("db/schema.rb"),
        b"ActiveRecord::Schema[7.1].define(version: 1) do\nend\n".to_vec(),
    );
    let err = ingest_app_from_tree(tree).expect_err("unknown DSL call is reported");
    assert!(format!("{err:?}").contains("routing_method"), "{err:?}");
}

#[test]
fn second_app_draw_appends_and_an_engine_draw_is_not_the_app() {
    let got = paths(&[(
        "config/routes.rb",
        r#"Rails.application.routes.draw do
  get "one", to: "pages#one"
end

Widgets::Engine.routes.draw do
  get "inside_engine", to: "pages#engine"
end

Rails.application.routes.draw do
  get "two", to: "pages#two"
end
"#,
    )]);
    assert_eq!(got, vec!["GET /one", "GET /two"]);
}

#[test]
fn load_of_a_routes_file_runs_it_at_top_scope_and_draw_inherits_the_scope() {
    let got = paths(&[
        (
            "config/routes.rb",
            r#"Rails.application.routes.draw do
  engine_root = Widgets::Engine.root
  load(engine_root.join("config", "routes", "services.rb").to_s)
  namespace :admin do
    draw(:team)
  end
end
"#,
        ),
        (
            "config/routes/services.rb",
            r#"Rails.application.routes.draw do
  namespace :services do
    get "status", to: "status#show"
  end
end
"#,
        ),
        ("config/routes/team.rb", "get \"members\", to: \"members#index\"\n"),
    ]);
    // `load` starts a fresh mapper (its own `draw` wrapper), so the
    // routes sit at the top; `draw(:team)` runs in the caller's scope.
    let mut got = got;
    got.sort();
    assert_eq!(got, vec!["GET /admin/members", "GET /services/status"]);
}

#[test]
fn nested_route_files_are_keyed_by_path_not_stem() {
    // Two files named `internal.rb` in different directories.
    let got = paths(&[
        (
            "config/routes.rb",
            r#"Rails.application.routes.draw do
  namespace :services do
    load(Support::Engine.root.join("config", "routes", "services", "internal.rb").to_s)
  end
  load(Support::Engine.root.join("config", "routes", "internal.rb").to_s)
end
"#,
        ),
        (
            "config/routes/services/internal.rb",
            "Rails.application.routes.draw do\n  get \"deep\", to: \"deep#show\"\nend\n",
        ),
        (
            "config/routes/internal.rb",
            "Rails.application.routes.draw do\n  get \"shallow\", to: \"shallow#show\"\nend\n",
        ),
    ]);
    assert_eq!(got, vec!["GET /deep", "GET /shallow"]);
}

#[test]
fn concerns_are_macros_replayed_at_each_call_site() {
    let flat = flat(&[(
        "config/routes.rb",
        r#"Rails.application.routes.draw do
  concern :commentable do |options|
    resources :comments, only: [:index], **options
  end
  concern :graphql_route do
    match :graphql, controller: :graphql, action: :query, via: [:post]
  end

  resources :posts, only: [], concerns: :commentable
  resources :pages, only: [] do
    concerns :commentable, prefix: "x"
  end
  namespace :api do
    concerns :graphql_route
  end
end
"#,
    )]);
    let got: Vec<String> = flat.iter().map(|(m, p, _)| format!("{m} {p}")).collect();
    assert_eq!(got, vec!["GET /posts/:post_id/comments", "GET /pages/:page_id/comments", "ANY /api/graphql"]);
}

#[test]
fn empty_only_list_means_no_actions() {
    let got = paths(&[(
        "config/routes.rb",
        r#"Rails.application.routes.draw do
  resources :things, only: [] do
    collection do
      get :search
    end
  end
  resources :nothing, only: []
end
"#,
    )]);
    assert_eq!(got, vec!["GET /things/search"]);
}

#[test]
fn controller_keyword_names_the_controller_of_a_bare_verb_route() {
    let flat = flat(&[(
        "config/routes.rb",
        r#"Rails.application.routes.draw do
  scope path: "oauth", as: "oauth" do
    get "authorize", controller: "oauth", action: :authorize
    post "grant", controller: "oauth"
  end
end
"#,
    )]);
    let got: Vec<String> = flat.iter().map(|(m, p, n)| format!("{m} {p} {n}")).collect();
    assert_eq!(got, vec!["GET /oauth/authorize oauth_authorize", "POST /oauth/grant oauth_grant"]);
}
