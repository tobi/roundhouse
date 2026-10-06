//! Object reflection and proven Module receivers keep their distinct protocols.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("rh_object_protocol_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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

const RECORD: &str = "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n";

#[test]
fn an_instance_answers_the_object_protocol() {
    let model = "class Widget < ApplicationRecord\n  def inst\n    self.instance_variable_get(:@x)\n    self.instance_variable_set(:@x, 1)\n    self.instance_variables.map(&:to_s)\n    self.instance_variable_defined?(:@x)\n    self.define_singleton_method(:x) { 1 }\n    self.singleton_class\n    self.in?([1])\n    self.to_json\n    self.dup\n    self.itself\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

#[test]
fn a_class_answers_the_module_protocol() {
    let model = "class Widget < ApplicationRecord\n  def self.go\n    self.define_method(:foo) { 1 }\n    self.alias_method(:a, :foo)\n    self.instance_methods.map(&:to_s)\n    self.method_defined?(:x)\n    self.const_get(:X)\n    self.instance_method(:x)\n    self.class_eval { 1 }\n    self.module_eval { 1 }\n    self.remove_method(:foo)\n    self.constants\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// Ruby's answers keep flowing: `instance_variables` is an Array of
/// Symbols, so `.upcase` (a String method) on an element is still a bug.
#[test]
fn the_answers_are_typed_not_untyped() {
    let model = "class Widget < ApplicationRecord\n  def inst\n    self.instance_variables.first.no_such_symbol_method\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains("no_such_symbol_method"), "{err}");
}

/// The fallback is for names the protocol owns; anything else is still
/// an unknown method.
#[test]
fn a_name_the_protocol_does_not_own_is_still_reported() {
    let model = "class Widget < ApplicationRecord\n  def inst\n    self.no_such_method_anywhere\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains("no_such_method_anywhere"), "{err}");
}
