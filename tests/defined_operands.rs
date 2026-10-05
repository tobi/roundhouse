//! `defined?` on operands other than a bareword.
//!
//! `defined?(Foo)` guards optional constants all over core
//! (`defined?(AdminAreaController)`, `defined?(Rails::Console)`), and
//! Method-call operands are preserved without evaluating the method.
//! Ingest once only knew barewords and ivars and dropped
//! the whole file on any other operand.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::emit::ruby;
use roundhouse::ingest::ingest_app_from_tree;

fn emit(body: &str) -> String {
    let files: Vec<(&str, String)> = vec![
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"accounts\" do |t|\n    t.string \"name\"\n  end\nend\n".to_string(),
        ),
        ("app/helpers/probe.rb", body.to_string()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    let app = ingest_app_from_tree(tree).expect("`defined?` on a supported operand must ingest");
    ruby::emit_library(&app)
        .iter()
        .find(|f| f.path.to_string_lossy().ends_with("probe.rb"))
        .map(|f| f.content.clone())
        .expect("probe.rb")
}

#[test]
fn a_constant_operand_is_kept() {
    let out = emit(
        r#"class Probe
  def admin?
    defined?(AdminAreaController)
  end
end
"#,
    );
    assert!(out.contains("defined?(AdminAreaController)"), "got:\n{out}");
}

#[test]
fn a_scoped_constant_operand_is_kept() {
    let out = emit(
        r#"class Probe
  def console?
    defined?(Rails::Console)
  end
end
"#,
    );
    assert!(out.contains("defined?(Rails::Console)"), "got:\n{out}");
}

#[test]
fn a_method_call_operand_is_kept() {
    let out = emit("class Probe\n  def responds?(obj)\n    defined?(obj.meth)\n  end\nend\n");
    assert!(out.contains("defined?(obj.meth)"), "got:\n{out}");
}
