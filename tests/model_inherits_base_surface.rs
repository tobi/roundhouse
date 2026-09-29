//! A model reaches the class-side and instance methods its app-defined
//! base model declares or includes.
//!
//! Core's `ApplicationRecord` includes concerns such as
//! `TransactionPreferReadCommitted` (whose `class_methods do` defines
//! `transaction_prefer_read_committed`). Every model inherits them, but
//! model `ClassInfo` carried no parent link, so `User.transaction_...`
//! was reported as an unknown method on `User`.

use std::process::Command;

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("rh_model_base_{}", std::process::id()));
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

#[test]
fn a_model_sees_its_base_models_concern_and_own_methods() {
    let err = check(&[
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\n  include Tprc\n  def self.zork; 1; end\n  def bork; 1; end\nend\n"),
        ("app/models/user.rb", "class User < ApplicationRecord\nend\n"),
        ("app/models/concerns/tprc.rb", "module Tprc\n  extend ActiveSupport::Concern\n\n  class_methods do\n    def tprc(**options)\n      1\n    end\n  end\nend\n"),
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\n  def index\n    User.tprc\n    User.zork\n    User.new.bork\n    head :ok\n  end\nend\n"),
        ("config/routes.rb", "Rails.application.routes.draw do\n  root to: \"application#index\"\nend\n"),
    ]);
    assert!(err.contains("0 error(s)"), "{err}");
}
