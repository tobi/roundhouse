//! `case … in` pattern matching.
//!
//! The IR models `case … when`, not Ruby's `in` patterns, so ingest
//! desugars a `case/in` into the `if` ladder it stands for: a temp for
//! the subject, one rung per `in`, and `NoMatchingPatternError` when
//! nothing matches and there is no `else`. Core uses array patterns with
//! constants and alternations (`in Ad, AdGroup | nil`), symbols
//! (`in [:phone_number, Taken]`) and hash patterns; before this the
//! CaseMatchNode dropped the whole file.
//!
//! The emitted Ruby is checked for shape AND run through Prism, so a
//! malformed ladder cannot pass on substring luck.

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
        ("app/helpers/matcher.rb", body.to_string()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    let app = ingest_app_from_tree(tree).expect("a `case/in` must ingest");
    let out = ruby::emit_library(&app)
        .iter()
        .find(|f| f.path.to_string_lossy().ends_with("matcher.rb"))
        .map(|f| f.content.clone())
        .expect("matcher.rb");
    let errors: Vec<String> =
        ruby_prism::parse(out.as_bytes()).errors().map(|e| e.message().to_string()).collect();
    assert!(errors.is_empty(), "emitted Ruby does not parse: {errors:?}\n{out}");
    out
}

#[test]
fn an_array_pattern_with_alternatives_becomes_an_if_ladder() {
    let out = emit(
        r#"class Matcher
  def check(level, parent)
    case [level, parent]
    in Ad, Ad | nil
      1
    in AdGroup, Campaign
      2
    else
      3
    end
  end
end
"#,
    );
    assert!(out.contains("is_a?(Array)"), "got:\n{out}");
    assert!(out.contains(".size == 2"), "got:\n{out}");
    assert!(out.contains("Ad === "), "got:\n{out}");
    assert!(out.contains(".nil?"), "got:\n{out}");
    assert!(out.contains("||"), "got:\n{out}");
    assert!(out.contains("elsif") || out.contains("else"), "got:\n{out}");
    assert!(!out.contains("NoMatchingPatternError"), "an else arm must not raise:\n{out}");
}

#[test]
fn without_an_else_a_miss_raises_no_matching_pattern_error() {
    let out = emit(
        r#"class Matcher
  def check(x)
    case x
    in Integer
      1
    end
  end
end
"#,
    );
    assert!(out.contains("raise NoMatchingPatternError.new("), "got:\n{out}");
}

#[test]
fn captures_and_binds_are_assigned_before_the_body() {
    let out = emit(
        r#"class Matcher
  def check(x)
    case x
    in [Integer => id, name]
      [id, name]
    else
      nil
    end
  end
end
"#,
    );
    assert!(out.contains("id = "), "got:\n{out}");
    assert!(out.contains("name = "), "got:\n{out}");
    assert!(out.contains("[0]") && out.contains("[1]"), "got:\n{out}");
}

#[test]
fn a_splat_binds_the_slice_between_the_fixed_elements() {
    let out = emit(
        r#"class Matcher
  def check(x)
    case x
    in [first, *middle, last]
      middle
    else
      nil
    end
  end
end
"#,
    );
    assert!(out.contains(".size >= 2"), "got:\n{out}");
    assert!(out.contains("middle = "), "got:\n{out}");
    assert!(out.contains("[-1]"), "got:\n{out}");
}

#[test]
fn a_hash_pattern_requires_its_keys_and_binds_the_shorthand() {
    let out = emit(
        r#"class Matcher
  def check(x)
    case x
    in { type: :card, amount: }
      amount
    else
      0
    end
  end
end
"#,
    );
    assert!(out.contains("is_a?(Hash)"), "got:\n{out}");
    assert!(out.contains("key?(:type)"), "got:\n{out}");
    assert!(out.contains("key?(:amount)"), "got:\n{out}");
    assert!(out.contains("amount = "), "got:\n{out}");
}

#[test]
fn a_guard_sees_the_bindings_and_a_failed_guard_falls_through() {
    let out = emit(
        r#"class Matcher
  def check(x)
    case x
    in [n] if n > 1
      :big
    in [_]
      :small
    else
      :none
    end
  end
end
"#,
    );
    assert!(out.contains("n = "), "got:\n{out}");
    assert!(out.contains("n > 1"), "got:\n{out}");
    assert!(out.contains(":big") && out.contains(":small") && out.contains(":none"), "got:\n{out}");
}

#[test]
fn a_pin_compares_with_case_equality() {
    let out = emit(
        r#"class Matcher
  def check(x, expected)
    case x
    in ^expected
      1
    else
      2
    end
  end
end
"#,
    );
    assert!(out.contains("expected === "), "got:\n{out}");
}
