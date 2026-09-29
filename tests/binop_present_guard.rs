//! `x.present?` / `x.blank?` as nil guards on a local.
//!
//! `id = params[:id]&.to_i; id.present? && id > 0` is the ubiquitous
//! Rails guard. A present value is never nil, so the true side of
//! `present?` (and the false side of `blank?`) drops the nil arm. The
//! other sides say nothing: `blank?` is also true for "" and false.

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};

fn binops(model: &str) -> Vec<String> {
    let tree: std::collections::HashMap<std::path::PathBuf, Vec<u8>> = [
        ("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base
end
"),
        ("app/models/thing.rb", model),
    ]
    .iter()
    .map(|(p, c)| (std::path::PathBuf::from(p), c.as_bytes().to_vec()))
    .collect();
    let mut app = roundhouse::ingest::ingest_app_from_tree(tree).expect("ingest tree");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .into_iter()
        .filter(|d| matches!(d.kind, DiagnosticKind::IncompatibleBinop { .. }))
        .map(|d| format!("{:?}", d.kind))
        .collect()
}

#[test]
fn present_and_blank_guard_a_nilable_local() {
    let found = binops(
        r#"class Thing < ApplicationRecord
  def a(x)
    v = x ? 1 : nil
    v.present? && v > 0
  end
  def b(x)
    v = x ? 1 : nil
    v.blank? || v > 0
  end
  def c(x)
    v = x ? 1 : nil
    return 0 if v.blank?
    v + 1
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn blank_side_of_present_does_not_narrow() {
    let found = binops(
        "class Thing < ApplicationRecord\n  def a(x)\n    v = x ? 1 : nil\n    v.present? || v > 0\n  end\nend\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
