//! A bare constant names the class nearest in the lexical nesting.
//!
//! Shopify core's login-with-shop controllers each declare their own
//! nested `CallbackParams` (`LoginWithShopController::CallbackParams`,
//! `FedcmController::CallbackParams`, ...) with different readers. The
//! typer resolved a bare `CallbackParams` by a registry-wide unique
//! suffix match, so as soon as a second controller declared the name the
//! match was ambiguous, the read stayed a bare `CallbackParams` that no
//! class is registered under, and every reader on it failed dispatch.
//! Ruby resolves the name from where it is written; so must the typer.

use std::process::Command;

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("rh_lexical_nested_{}", std::process::id()));
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

fn controller(name: &str, reader: &str) -> String {
    format!(
        "module Auth\n  class {name}Controller < ApplicationController\n    class CallbackParams\n      attr_reader :{reader}\n      def initialize({reader}:)\n        @{reader} = {reader}\n      end\n    end\n\n    def index\n      params = callback_params\n      params.{reader}\n      head :ok\n    end\n\n    def callback_params\n      @callback_params = CallbackParams.new({reader}: 1)\n    end\n  end\nend\n"
    )
}

#[test]
fn same_named_nested_classes_resolve_to_their_own_scope() {
    let foo = controller("Foo", "state");
    let bar = controller("Bar", "context_code");
    let err = check(&[
        ("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n"),
        ("app/controllers/foo_controller.rb", &foo),
        ("app/controllers/bar_controller.rb", &bar),
        ("config/routes.rb", "Rails.application.routes.draw do\n  get \"/foo\", to: \"auth/foo#index\"\n  get \"/bar\", to: \"auth/bar#index\"\nend\n"),
    ]);
    assert!(err.contains("0 error(s)"), "{err}");
}
