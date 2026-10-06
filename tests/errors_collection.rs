//! Errors operations without a shared implementation remain named refusals.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;

fn dispatch_failures(action: &str) -> Vec<String> {
    let model = format!(
        "class Checkout\n  include ActiveModel::Validations\n\n  def check(other)\n{action}\n  end\nend\n"
    );
    let tree: HashMap<PathBuf, Vec<u8>> = [("app/models/checkout.rb", model)]
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    let mut app = ingest_app_from_tree(tree).expect("ingest");
    let residue = roundhouse::session::analyze_and_lower(&mut app);
    residue
        .iter()
        .chain(roundhouse::analyze::diagnose(&app).iter())
        .map(roundhouse::diagnostic::Diagnostic::to_string)
        .filter(|d| d.contains("send_dispatch_failed") || d.contains("unsupported"))
        .collect()
}

#[test]
fn unimplemented_errors_operations_are_refused() {
    let diags = dispatch_failures(
        "    errors.added?(:base, :blank)\n    errors.import(other.errors.first)\n    errors.merge!(other.errors)\n    errors.delete(:base)\n    errors.where(:base).any?\n    errors.details[:base]\n    errors.map(&:type).include?(:x)\n    errors.errors[0].options[:code]\n    errors.attribute_names\n    errors.messages",
    );
    for name in ["added?", "import", "merge!", "delete", "where", "details", "attribute_names"] {
        assert!(diags.iter().any(|d| d.contains(name)), "missing {name}: {diags:?}");
    }
}
