//! `new` inside a class-side method builds an instance of the class
//! that received the call.
//!
//! Shopify core's `YamlDb` keeps its memoized factory on the singleton
//! (`def load; @instance ||= new(file_path); end`) and `CurrencyDb`,
//! `CryptoCurrencyDb`, ... subclass it and add readers. `self` in
//! `load` is whichever subclass was called, but the typer answered the
//! class the `def` sits in, so `CurrencyDb.load.map { ... }` and
//! `CurrencyDb.load[code]` were reported as unknown methods on `YamlDb`.

use std::process::Command;

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("rh_class_side_new_{}", std::process::id()));
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

const BASE: &str = "class YamlDb\n  class << self\n    def load\n      @instance ||= new(\"x\")\n    end\n\n    def fresh\n      new(\"y\")\n    end\n  end\n\n  def initialize(path)\n    @path = path\n  end\nend\n";
const SUB: &str = "class CurrencyDb < YamlDb\n  def codes\n    [\"USD\"]\n  end\nend\n";
const CONTROLLER: &str = "class ApplicationController < ActionController::Base\n  def index\n    CurrencyDb.load.codes\n    CurrencyDb.fresh.codes\n    head :ok\n  end\nend\n";
const ROUTES: &str = "Rails.application.routes.draw do\n  root to: \"application#index\"\nend\n";

#[test]
fn a_subclass_factory_answers_the_subclass() {
    let err = check(&[
        ("app/models/yaml_db.rb", BASE),
        ("app/models/currency_db.rb", SUB),
        ("app/controllers/application_controller.rb", CONTROLLER),
        ("config/routes.rb", ROUTES),
    ]);
    assert!(err.contains("0 error(s)"), "{err}");
}

/// The instance a class-side `new` builds is still an instance of the
/// class the `def` sits in, for the body that builds it: core's
/// `ClientDetails.from_checkout_client_details` assigns attributes onto
/// `details = new`, and `UriValidator.verify` runs `new(...).tap(&:valid?)`.
/// Each of those sends was reported as "no known method on instance".
#[test]
fn the_built_instance_answers_the_defining_class_inside_the_factory() {
    let schema = "ActiveRecord::Schema.define do\n  create_table \"client_details\" do |t|\n    t.string \"user_agent\"\n  end\nend\n";
    let record = "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n";
    let model = "class ClientDetails < ApplicationRecord\n  def self.from_other(agent)\n    details = new\n    details.user_agent = agent\n    details\n  end\nend\n";
    let validator = "class UriValidator\n  include ActiveModel::Validations\n  def initialize(uri_string:)\n    @uri_string = uri_string\n  end\n  class << self\n    #: (uri_string: String) -> UriValidator\n    def verify(uri_string:)\n      new(uri_string:).tap(&:valid?)\n    end\n  end\nend\n";
    let err = check(&[
        ("db/schema.rb", schema),
        ("app/models/application_record.rb", record),
        ("app/models/client_details.rb", model),
        ("app/models/uri_validator.rb", validator),
    ]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}
