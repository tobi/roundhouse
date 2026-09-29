//! RBS `type name = ...` aliases, in `.rbs` files and in Sorbet's
//! comment form (`#: type object_type = ::User`).
//!
//! A signature that names an alias could not be read at all: the
//! reader stopped at the alias and dropped the whole method signature,
//! so the method was typed as nothing. Shopify core declares about 360
//! of them and uses them in some 940 signatures.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};
use roundhouse::ident::{ClassId, Symbol};
use roundhouse::ingest::ingest_app_from_tree;
use roundhouse::ty::Ty;

fn dispatch_failures(files: &[(&str, &str)]) -> Vec<String> {
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    Analyzer::new(&app).analyze(&mut app);
    let mut out: Vec<String> = diagnose(&app)
        .into_iter()
        .filter_map(|d| match d.kind {
            DiagnosticKind::SendDispatchFailed { method, .. } => Some(method.as_str().to_string()),
            _ => None,
        })
        .collect();
    out.sort();
    out
}

#[test]
fn an_alias_in_an_rbs_file_stands_for_the_type_it_names() {
    let sigs = roundhouse::rbs::parse_app_signatures(
        "class Cart\n  type path = Array[Integer]\n  type keyed = Hash[Symbol, path]\n  def walk: (path, keyed) -> path\nend\n",
    )
    .expect("parse");
    let walk = &sigs[&ClassId(Symbol::new("Cart"))][&Symbol::new("walk")];
    let Ty::Fn { params, ret, .. } = walk else { panic!("{walk:?}") };
    let ints = Ty::Array { elem: Box::new(Ty::Int) };
    assert_eq!(params[0].ty, ints);
    assert_eq!(
        params[1].ty,
        Ty::Hash { key: Box::new(Ty::Sym), value: Box::new(ints.clone()) },
        "an alias may name another declared after it"
    );
    assert_eq!(**ret, ints);
}

#[test]
fn an_alias_nobody_declared_still_leaves_the_signature_unread() {
    let sigs = roundhouse::rbs::parse_signatures("class Cart\n  def walk: (nowhere) -> void\nend\n");
    assert!(sigs.is_err(), "{sigs:?}");
}

const SCHEMA: (&str, &str) = ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n");

/// The comment form, as core writes it: the alias in the class body
/// (one of them continued across `#|` lines), used by a signature
/// below and by a method in a class nested inside.
#[test]
fn a_comment_alias_types_the_parameters_of_the_signatures_that_use_it() {
    let failures = dispatch_failures(&[
        SCHEMA,
        ("app/models/line.rb", "class Line\n  def cents\n    1\n  end\nend\n"),
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController < ActionController::Base\nend\n",
        ),
        (
            "app/controllers/cart_controller.rb",
            r#"class CartController < ApplicationController
  #: type lines = Array[Line]
  #: type by_key = Hash[
  #|   Symbol,
  #|   Line
  #| ]

  #: (lines, by_key) -> Integer
  def totals(lines, by_key)
    lines.each { |l| l.cents.bogus_from_alias }
    by_key.each { |_k, l| l.cents.bogus_from_multiline_alias }
    1
  end

  def use
    totals([], {}).bogus_from_return
  end
end
"#,
        ),
    ]);
    assert_eq!(
        failures,
        vec![
            "bogus_from_alias".to_string(),
            "bogus_from_multiline_alias".to_string(),
            "bogus_from_return".to_string(),
        ]
    );
}
