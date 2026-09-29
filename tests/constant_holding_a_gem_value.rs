//! A constant that HOLDS a gem value is a value, not a class name.
//!
//! `MIN_PRICE = Money.new(1, "USD")` reads as `Class { Money }` (the type
//! does not tell a class object from an instance), and `Money` is a gem
//! class the app never registered. Sends on a value of an unregistered
//! class are an unmodelled gem boundary and answer untyped; sends on a
//! WRITTEN class name (`Money.zero`) are not, because the app named the
//! class and the class is missing. The typer told them apart by "is the
//! receiver a Const", so `MIN_PRICE.currency` and `MIN_PRICE.value` were
//! reported as unknown methods of `Money`. A class object is written as
//! the class it is, so the check is now whether the written name is the
//! class's own.

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
fn a_constant_holding_a_gem_value_is_a_gem_boundary() {
    let model = "class Widget < ApplicationRecord\n  MIN_PRICE = Money.new(1, \"USD\")\n\n  def price\n    MIN_PRICE.currency\n    MIN_PRICE.value.positive?\n    Widget::MIN_PRICE.value\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// A written class name is still the class object: the gem class is
/// unregistered, so a constant read of it is not a value.
#[test]
fn a_written_class_name_is_still_a_class_object() {
    let model = "class Widget < ApplicationRecord\n  def price\n    Money.no_such_class_method\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(!err.contains("panicked"), "{err}");
}
