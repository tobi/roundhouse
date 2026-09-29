//! A method that only raises `NotImplementedError` is an abstract hook.
//!
//! Shopify core's concerns declare the seams an includer fills in as
//! `def set_class; raise NotImplementedError, "..."; end`. The body never
//! returns, so the typer harvested `Bottom`, and every call on the
//! result (`set_class.new(...)`, `record.send(:nameserver_payload)`
//! mapped over a relation) was reported as an unknown method on `bot`.
//! What the hook returns is what the override returns, which the concern
//! cannot see: gradual, not diverging.

use std::process::Command;

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("rh_abstract_hook_{}", std::process::id()));
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

const CONTROLLER: &str = "class ApplicationController < ActionController::Base\n  def index\n    set = set_class.new(1)\n    set.rows\n    head :ok\n  end\n\n  def set_class\n    raise NotImplementedError, \"must implement\"\n  end\nend\n";
const ROUTES: &str = "Rails.application.routes.draw do\n  root to: \"application#index\"\nend\n";

#[test]
fn a_raise_not_implemented_hook_is_not_bottom() {
    let err = check(&[
        ("app/controllers/application_controller.rb", CONTROLLER),
        ("config/routes.rb", ROUTES),
    ]);
    assert!(err.contains("0 error(s)"), "{err}");
}
