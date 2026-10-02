//! A source constant holding a value from an undeclared gem remains
//! unsupported. It must not become a phantom class or turn its readers
//! into clean support without a declaration and runtime implementation.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("rh_const_gem_value_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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
fn a_value_from_an_undeclared_gem_constant_is_unsupported() {
    let model = "class Widget < ApplicationRecord\n  MIN_PRICE = Money.new(1, \"USD\")\n\n  def price\n    MIN_PRICE.currency\n    MIN_PRICE.value.positive?\n    Widget::MIN_PRICE.value\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains("error[unsupported]"), "{err}");
    assert!(err.contains("Widget::MIN_PRICE"), "{err}");
    assert!(!err.contains("send_dispatch_failed"), "{err}");
}

/// A written class name is still the class object: the gem class is
/// unregistered, so a constant read of it is not a value.
#[test]
fn a_written_class_name_is_still_a_class_object() {
    let model = "class Widget < ApplicationRecord\n  def price\n    Money.no_such_class_method\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(!err.contains("panicked"), "{err}");
    assert!(err.contains("constant not supported (all targets): Money"), "{err}");
}
