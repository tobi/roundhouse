//! `&expr` where `expr` is any proc-valued expression, not just a
//! `&:symbol` or a `&local`.
//!
//! Ruby evaluates the operand once, calls `to_proc` on it and hands the
//! result to the callee as its block. Core uses the whole range:
//! `items.map(&method(:one))`, `instance_exec(&SORT_ORDERS[key])`,
//! `instance_eval(&self.class.options[attr][:packer])`,
//! `instance_exec(&guard.predicate)`. Ingest used to refuse everything
//! but the two simple forms, which dropped the ENTIRE file (its model,
//! and with it every call other files make into it) rather than one
//! statement.

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
        ("app/helpers/filter.rb", body.to_string()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    let app = ingest_app_from_tree(tree).expect("a `&expr` block argument must ingest");
    ruby::emit_library(&app)
        .iter()
        .find(|f| f.path.to_string_lossy().ends_with("filter.rb"))
        .map(|f| f.content.clone())
        .expect("filter.rb")
}

#[test]
fn a_method_object_is_forwarded_as_the_block() {
    let out = emit(
        r#"class Filter
  def attrs(items)
    items.map(&method(:one))
  end

  def one(x)
    x.to_s
  end
end
"#,
    );
    assert!(out.contains("items.map(&method(:one))"), "got:\n{out}");
}

#[test]
fn an_indexed_or_ivar_or_chained_proc_is_forwarded() {
    let out = emit(
        r#"class Filter
  ORDERS = { a: -> { 1 } }.freeze

  def run(key, guard)
    instance_exec(&ORDERS[key])
    instance_exec(&@callback)
    instance_exec(&guard.predicate)
  end
end
"#,
    );
    assert!(out.contains("instance_exec(&ORDERS[key])"), "got:\n{out}");
    assert!(out.contains("instance_exec(&@callback)"), "got:\n{out}");
    assert!(out.contains("instance_exec(&guard.predicate)"), "got:\n{out}");
}

#[test]
fn forwarded_ivar_and_call_result_execute_once() {
    let out = emit(r#"class Filter
  def initialize
    @calls = 0
    @callback = ->(x) { x * 2 }
  end

  def compute(n)
    @calls += 1
    ->(x) { x + n }
  end

  def run
    doubled = [1, 2].map(&@callback)
    added = [1, 2].map(&compute(3))
    [doubled, added, @calls]
  end
end
"#);
    let script = format!("{out}\nraise 'block forwarding changed' unless Filter.new.run == [[2, 4], [4, 5], 1]\n");
    let result = std::process::Command::new("ruby").args(["-e", &script]).output().unwrap();
    assert!(result.status.success(), "{}\n{script}", String::from_utf8_lossy(&result.stderr));
}
