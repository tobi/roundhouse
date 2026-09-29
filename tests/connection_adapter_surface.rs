//! `connection` answers the adapter surface Rails documents, not only the
//! calls the runtime implements.
//!
//! The registered `ActiveRecord::Connection` is read from the runtime's
//! own signatures (`exec_query`, `quote`, `execute`, ...). Code that
//! reaches for the adapter directly also calls `quote_column_name`,
//! `quote_table_name`, `transaction_open?`, `select_rows`, `pool` ...,
//! which Rails' `AbstractAdapter` defines and the runtime never needed.
//! Each was reported as an unknown method of the connection.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("rh_connection_adapter_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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
fn the_connection_answers_the_adapter_surface() {
    let model = "class Widget < ApplicationRecord\n  def self.go\n    c = self.connection\n    c.quote_column_name(\"x\").upcase\n    c.quote_table_name(\"t\")\n    c.transaction_open?\n    c.select_rows(\"select 1\").first\n    c.pool\n    c.current_transaction\n    c.tables.first.upcase\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// A name the adapter does not have is still a visible gap.
#[test]
fn a_name_the_adapter_lacks_is_still_reported() {
    let model = "class Widget < ApplicationRecord\n  def self.go\n    self.connection.no_such_adapter_method\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/widget.rb", model)]);
    assert!(err.contains("no_such_adapter_method"), "{err}");
}
