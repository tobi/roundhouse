//! `for x in xs … end`.
//!
//! Ruby expands a `for` loop into a call to `each` on the collection, so
//! it ingests as exactly that: `xs.each do |x| … end`. Core's diagnostics
//! views loop over `for order_transaction in billing_attempt.order_transactions`,
//! and the unsupported node dropped the whole template.

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
        ("app/helpers/loops.rb", body.to_string()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    let app = ingest_app_from_tree(tree).expect("a `for` loop must ingest");
    ruby::emit_library(&app)
        .iter()
        .find(|f| f.path.to_string_lossy().ends_with("loops.rb"))
        .map(|f| f.content.clone())
        .expect("loops.rb")
}

#[test]
fn a_for_loop_is_an_each_call_with_the_variable_as_block_parameter() {
    let out = emit(
        r#"class Loops
  def show(items)
    for item in items
      puts item
    end
  end
end
"#,
    );
    assert!(out.contains("items.each do |item|"), "got:\n{out}");
    assert!(out.contains("puts(item)") || out.contains("puts item"), "got:\n{out}");
}

#[test]
fn several_loop_variables_destructure_like_block_parameters() {
    let out = emit(
        r#"class Loops
  def show(pairs)
    for key, value in pairs
      puts key, value
    end
  end
end
"#,
    );
    assert!(out.contains("pairs.each do |key, value|"), "got:\n{out}");
}

#[test]
fn break_and_next_carry_over_unchanged() {
    let out = emit(
        r#"class Loops
  def first_even(items)
    for n in items
      next if n.odd?
      break
    end
  end
end
"#,
    );
    assert!(out.contains("items.each do |n|"), "got:\n{out}");
    assert!(out.contains("next"), "got:\n{out}");
    assert!(out.contains("break"), "got:\n{out}");
}
