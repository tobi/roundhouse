//! An Integer answers the `Comparable` / `Numeric` / `Integer` protocol
//! Ruby gives it, not only arithmetic.
//!
//! `page.clamp(1, 100)`, `n.between?(1, 5)`, `1.upto(n)`, `n.to_int`,
//! `n.to_d`, `5.megabyte` are stdlib / ActiveSupport methods every
//! Integer has. The Integer table held arithmetic and the plural
//! byte-size helpers, so each of these was reported as an unknown method
//! of Integer in core's controllers.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("rh_integer_protocol_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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
fn an_integer_answers_the_comparable_and_numeric_protocol() {
    let model = "class Widget < ApplicationRecord\n  def go\n    n = 5\n    n.clamp(1, 100) + 1\n    n.between?(1, 9)\n    n.to_int + 1\n    n.divmod(2).first\n    n.fdiv(2).round\n    n.to_d\n    n.megabyte\n    n.upto(9) { |i| i }\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// The answers are typed: `clamp` is an Integer, so a String method on
/// it is still a bug.
#[test]
fn the_answers_are_typed() {
    let model = "class Widget < ApplicationRecord\n  def go\n    n = 5\n    n.clamp(1, 100).upcase\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains("upcase"), "{err}");
}
