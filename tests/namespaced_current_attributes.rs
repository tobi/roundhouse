//! `Current.user` inside `module DeveloperDashboard`, where
//! `DeveloperDashboard::Current < ActiveSupport::CurrentAttributes`
//! declares `attribute :user` AND overrides the reader with a method
//! that calls `super` (raise if unset). Core has dozens of `Current`
//! classes; each namespace's reads must resolve to its own.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

const CURRENT: &str = r#"module Dash
  class Current < ActiveSupport::CurrentAttributes
    attribute :organization
    attribute :user

    #: -> String
    def organization
      raise "unset" unless attributes[:organization]

      super
    end

    #: -> String
    def user
      super
    end
  end
end
"#;

const OTHER: &str = r#"module Other
  class Current < ActiveSupport::CurrentAttributes
    attribute :flag
  end
end
"#;

fn diagnostics() -> Vec<String> {
    let files: Vec<(&str, String)> = vec![
        ("db/schema.rb", "ActiveRecord::Schema.define do\n  create_table \"users\", force: :cascade do |t|\n    t.string \"name\"\n  end\nend\n".into()),
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\nend\n".into()),
        ("app/models/dash/current.rb", CURRENT.into()),
        ("app/models/other/current.rb", OTHER.into()),
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n".into()),
        ("app/controllers/dash/keys_controller.rb", "module Dash\n  class KeysController < ApplicationController\n    def index\n      Current.user.upcase\n      Current.organization.upcase\n    end\n  end\nend\n".into()),
        ("config/routes.rb", "Rails.application.routes.draw do\n  get \"/keys\", to: \"dash/keys#index\"\nend\n".into()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> =
        files.into_iter().map(|(p, c)| (PathBuf::from(p), c.into_bytes())).collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    let residue = roundhouse::session::analyze_and_lower(&mut app);
    residue
        .iter()
        .chain(roundhouse::analyze::diagnose(&app).iter())
        .map(roundhouse::diagnostic::Diagnostic::to_string)
        .collect()
}

#[test]
fn a_namespaced_current_answers_its_own_attribute_readers() {
    let diags = diagnostics();
    assert!(!diags.iter().any(|d| d.contains("send_dispatch_failed")), "{diags:?}");
}
