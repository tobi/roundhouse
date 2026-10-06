//! Non-nil assertion names need an app implementation or a shared nil-check lowerer.

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
        .filter(|d| d.contains("send_dispatch_failed") || d.contains("unsupported"))
        .collect()
}

#[test]
fn universal_not_nil_bang_is_refused() {
    let diags = dispatch_failures("    name = User.first.name\n    name.not_nil!.upcase\n    [name].compact.first.not_nil!.strip");
    assert!(diags.iter().any(|d| d.contains("not_nil!")), "{diags:?}");
}

