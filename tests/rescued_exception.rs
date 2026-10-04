//! Unresolved rescue classes remain blocking constant errors. Their unknown
//! bindings must not invent StandardError and produce method-error cascades.
//! Resolved exception classes retain their checked Exception surface.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUN: AtomicUsize = AtomicUsize::new(0);

fn check(files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("rh_rescued_exception_{}_{}", std::process::id(), RUN.fetch_add(1, Ordering::SeqCst)));
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

fn service(rescue_line: &str, body: &str) -> String {
    format!("class Charger < ApplicationRecord\n  def go\n    raise \"x\"\n  {rescue_line}\n{body}\n  end\nend\n")
}

#[test]
fn an_undeclared_gem_error_reports_the_constant_without_binding_cascades() {
    let src = service(
        "rescue Stripe::CardError => e",
        "    e.response\n    e.status\n    e.http_body.to_s\n    e.message.upcase",
    );
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/charger.rb", &src)]);
    assert!(err.contains("constant not supported (all targets): Stripe::CardError"), "{err}");
    assert!(!err.contains("send_dispatch_failed"), "{err}");
    assert!(err.contains(" 1 error(s)"), "{err}");
}

#[test]
fn a_union_with_an_undeclared_gem_error_keeps_the_constant_error() {
    let src = service(
        "rescue ArgumentError, Stripe::CardError => e",
        "    e.message.upcase\n    e.cause",
    );
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/charger.rb", &src)]);
    assert!(err.contains("constant not supported (all targets): Stripe::CardError"), "{err}");
    assert!(!err.contains("send_dispatch_failed"), "{err}");
    assert!(err.contains(" 1 error(s)"), "{err}");
}

/// `cause` is the exception being handled when this one was raised, or
/// nil; the rest of the Exception surface is typed, not untyped.
#[test]
fn the_exception_surface_includes_cause() {
    let src = service(
        "rescue StandardError => e",
        "    e.cause&.message\n    e.backtrace_locations\n    e.detailed_message.upcase\n    e.exception",
    );
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/charger.rb", &src)]);
    assert!(err.contains(" 0 error(s)"), "{err}");
}

/// A registered exception is still checked against its own surface.
#[test]
fn a_registered_exception_is_still_checked() {
    let src = service("rescue ArgumentError => e", "    e.no_such_exception_method");
    let err = check(&[("app/models/application_record.rb", RECORD), ("app/models/charger.rb", &src)]);
    assert!(err.contains("no_such_exception_method"), "{err}");
}
