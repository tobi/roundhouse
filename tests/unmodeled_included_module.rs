//! A class that includes a module the app never registered has methods
//! the analyzer cannot see.
//!
//! Core's `ApiClient` says `include ::ApiVersioning::ApiClient`; the
//! gem's module answers `ApiClient.current` and friends (through
//! `mixes_in_class_methods` / an `included` hook that extends the
//! class). The analyzer walked the class, its registered includes and its
//! superclasses, found no `current`, and reported it as an unknown method
//! of `ApiClient`, a hundred times over. An unregistered superclass
//! already made the walk answer untyped (the method is most likely
//! inherited from it); an unregistered included module says the same.
//! Ruby's own and the frameworks' modules are known ground and do not.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("rh_unmodeled_included_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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
const CLIENT: &str = "class ApiClient < ApplicationRecord\n  include ::ApiVersioning::ApiClient\nend\n";

#[test]
fn a_method_from_an_unregistered_included_module_is_a_boundary() {
    let caller = "class Caller < ApplicationRecord\n  def go\n    ApiClient.current.id\n    ApiClient.new.scope_permissions\n  end\nend\n";
    let err = check(&[
        ("app/models/application_record.rb", RECORD),
        ("app/models/api_client.rb", CLIENT),
        ("app/models/caller.rb", caller),
    ]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// Ruby's own modules are known ground: including `Comparable` does not
/// make every missing method a maybe.
#[test]
fn a_ruby_module_is_known_ground() {
    let other = "class Other < ApplicationRecord\n  include Comparable\n  def go\n    self.no_such_method_here\n  end\nend\n";
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/other.rb", other)]);
    assert!(err.contains("no_such_method_here"), "{err}");
}

/// The class's own methods still win over the escape.
#[test]
fn a_registered_method_keeps_its_own_type() {
    let client = "class ApiClient < ApplicationRecord\n  include ::ApiVersioning::ApiClient\n  def label\n    \"x\"\n  end\nend\n";
    let caller = "class Caller < ApplicationRecord\n  def go\n    ApiClient.new.label.no_such_string_method\n  end\nend\n";
    let err = check(&[
        ("app/models/application_record.rb", RECORD),
        ("app/models/api_client.rb", client),
        ("app/models/caller.rb", caller),
    ]);
    assert!(err.contains("no_such_string_method"), "{err}");
}
