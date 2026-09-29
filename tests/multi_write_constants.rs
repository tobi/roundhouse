//! `A, B = pair` with constant targets.
//!
//! A multiple assignment may destructure into constants as well as
//! locals; core's Domain model does `SSL_PENDING, SSL_PROVISIONED,
//! SSL_FAILED, SSL_EXTERNAL = SSL_STATES.map(&:freeze)` in its class body.
//! The target was refused, and the refusal dropped the whole model.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;
use roundhouse::dialect::ModelBodyItem;
use roundhouse::expr::{Expr, ExprNode, LValue};

/// The class body's `Unknown` statements, as ingested.
fn class_body_exprs(body: &str) -> Vec<Expr> {
    let files: Vec<(&str, String)> = vec![
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"accounts\" do |t|\n    t.string \"name\"\n  end\nend\n".to_string(),
        ),
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n".to_string(),
        ),
        ("app/models/account.rb", format!("class Account < ApplicationRecord\n{body}\nend\n")),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    let app = ingest_app_from_tree(tree).expect("constant multi-write targets must ingest");
    let model = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Account")
        .expect("the Account model must survive ingest");
    model
        .body
        .iter()
        .filter_map(|item| match item {
            ModelBodyItem::Unknown { expr, .. } => Some(expr.clone()),
            _ => None,
        })
        .collect()
}

fn const_targets(e: &Expr) -> Option<Vec<String>> {
    let paths = |targets: &[LValue]| {
        targets
            .iter()
            .map(|t| match t {
                LValue::Const { path } => Some(
                    path.iter().map(|s| s.as_str().to_string()).collect::<Vec<_>>().join("::"),
                ),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
    };
    match &*e.node {
        ExprNode::MultiAssign { targets, .. } => paths(targets),
        ExprNode::Seq { exprs } => {
            let assigned: Option<Vec<String>> = exprs
                .iter()
                .filter_map(|x| match &*x.node {
                    ExprNode::Assign { target: LValue::Const { path }, .. } => {
                        Some(Some(path.iter().map(|s| s.as_str().to_string()).collect::<String>()))
                    }
                    _ => None,
                })
                .collect();
            assigned.filter(|a| !a.is_empty())
        }
        _ => None,
    }
}

#[test]
fn constants_are_destructured_from_the_right_hand_side() {
    let exprs = class_body_exprs("  PENDING, PROVISIONED = %w[a b].map(&:freeze)\n");
    let targets = exprs.iter().find_map(const_targets).expect("a constant multi-write");
    assert_eq!(targets, vec!["PENDING".to_string(), "PROVISIONED".to_string()]);
}

#[test]
fn a_splat_after_constant_targets_takes_the_rest() {
    let exprs = class_body_exprs("  FIRST, *OTHERS = %w[a b c]\n");
    let targets = exprs.iter().find_map(const_targets).expect("a constant multi-write");
    assert_eq!(targets, vec!["FIRST".to_string(), "OTHERS".to_string()]);
}

#[test]
fn qualified_constants_keep_their_scope() {
    let exprs = class_body_exprs("  Foo::A, Foo::B = 1, 2\n");
    let targets = exprs.iter().find_map(const_targets).expect("a constant multi-write");
    assert_eq!(targets, vec!["Foo::A".to_string(), "Foo::B".to_string()]);
}
