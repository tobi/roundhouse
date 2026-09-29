//! `session` is an `ActionDispatch::Request::Session`, not a
//! `Hash[String, String]`: it answers `id`, `options`, `loaded?`, and
//! whatever an app mixes onto it (core's `session.essential`).

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

fn dispatch_failures(action: &str) -> Vec<String> {
    let controller = format!(
        "class UsersController < ApplicationController\n  def index\n{action}\n  end\nend\n"
    );
    let files: Vec<(&str, String)> = vec![
        ("db/schema.rb", "ActiveRecord::Schema.define do\n  create_table \"users\", force: :cascade do |t|\n    t.string \"name\"\n  end\nend\n".into()),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n".into()),
        ("app/models/user.rb", "class User < ApplicationRecord\nend\n".into()),
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n".into()),
        ("app/controllers/users_controller.rb", controller),
        ("config/routes.rb", "Rails.application.routes.draw do\n  get \"/users\", to: \"users#index\"\nend\n".into()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> =
        files.into_iter().map(|(p, c)| (PathBuf::from(p), c.into_bytes())).collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    let residue = roundhouse::session::analyze_and_lower(&mut app);
    residue
        .iter()
        .chain(roundhouse::analyze::diagnose(&app).iter())
        .map(roundhouse::diagnostic::Diagnostic::to_string)
        .filter(|d| d.contains("send_dispatch_failed"))
        .collect()
}

#[test]
fn session_answers_its_own_surface() {
    let diags = dispatch_failures("    session.id\n    session.options[:skip] = true\n    session.clear\n    session[:user_id] = 1\n    session.loaded?\n    session.essential");
    assert!(diags.is_empty(), "{diags:?}");
}


#[test]
fn a_session_read_stays_a_nilable_string() {
    // The blank-predicate grounding of `(rd = session[:k]).present?`
    // needs a nilable String receiver.
    let diags = dispatch_failures("    rd = session[:redirect_to]\n    rd.upcase if rd.present?");
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn rack_request_accessors_dispatch() {
    let diags = dispatch_failures("    request.POST\n    request.original_fullpath.upcase\n    request.get_header(\"HTTP_X\")\n    request.request_method_symbol");
    assert!(diags.is_empty(), "{diags:?}");
}
