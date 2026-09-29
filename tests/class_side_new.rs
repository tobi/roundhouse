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
