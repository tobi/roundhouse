//! `T.unsafe(x)` and `x #: as untyped` take the value out of the type
//! system.
//!
//! Both are the author's signed statement that the receiver's type is not
//! to be trusted from here: core writes `T.unsafe(self).define_method(...)`
//! and, in RBS style,
//!
//! ```text
//! self #: as untyped
//!   .before_update(prepend: true) { ... }
//! ```
//!
//! Ingest used to keep only the value, so the sends were checked against
//! the model and reported as unknown methods. A `#: as T` comment that
//! ends the receiver's line and is followed by a leading-dot line is now
//! read as the receiver's assertion, and `untyped` counts as a reading.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("rh_untyped_ascription_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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
fn t_unsafe_leaves_the_type_system() {
    let model = "class Widget < ApplicationRecord\n  def self.go\n    T.unsafe(self).not_a_real_method(:foo) { 1 }\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

#[test]
fn an_untyped_assertion_before_a_leading_dot_line_applies_to_the_receiver() {
    let model = "class Widget < ApplicationRecord\n  def self.go\n    self #: as untyped\n      .not_a_real_method(prepend: true) { 1 }\n  end\n\n  def inst\n    self #: as untyped\n      &.also_not_real\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// The comment only asserts when the call continues on the next line: a
/// method that really does not exist, called on its own line, is still
/// reported.
#[test]
fn an_unrelated_comment_asserts_nothing() {
    let model = "class Widget < ApplicationRecord\n  def inst\n    self.not_a_real_method\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains("not_a_real_method"), "{err}");
}
