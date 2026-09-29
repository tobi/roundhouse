//! A model answers the schema-reflection half of `ActiveRecord::Base`.
//!
//! `columns`, `sanitize_sql_like`, `base_class`, `descendants`,
//! `table_exists?` ... are class methods every Active Record model
//! inherits, but only the query half (`where`, `find`, `column_names`,
//! `columns_hash`) was registered. Core reads `columns` in
//! `ModelCaching` and `Delivery`, and the `sanitize_sql*` family in its
//! search scopes; each was reported as an unknown method of the model.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("rh_model_schema_reflection_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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
fn a_model_answers_columns_and_the_sanitizers() {
    let model = "class Widget < ApplicationRecord\n  def self.report\n    names = self.columns.sort_by(&:name).map(&:name)\n    like = self.sanitize_sql_like(\"50%\")\n    arr = self.sanitize_sql([\"a = ?\", 1])\n    [names, like, arr]\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

#[test]
fn a_model_answers_hierarchy_and_table_reflection() {
    let model = "class Widget < ApplicationRecord\n  def self.report\n    self.base_class.name\n    self.descendants.map(&:name)\n    self.subclasses.size\n    self.table_exists?\n    self.abstract_class?\n    self.inheritance_column.upcase\n    self.quoted_table_name.upcase\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// The registration is per model, not a blanket escape: a method Active
/// Record does not have is still reported.
#[test]
fn a_method_active_record_does_not_have_is_still_reported() {
    let model = "class Widget < ApplicationRecord\n  def self.report\n    self.no_such_reflection\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains("no_such_reflection"), "{err}");
}
