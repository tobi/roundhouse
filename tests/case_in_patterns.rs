//! `case … in` pattern matching.
//!
//! Native structural-pattern IR preserves Ruby's deconstruction and
//! binding semantics, including `NoMatchingPatternError` when
//! nothing matches and there is no `else`. Core uses array patterns with
//! constants and alternations (`in Ad, AdGroup | nil`), symbols
//! (`in [:phone_number, Taken]`) and hash patterns; before this the
//! CaseMatchNode dropped the whole file.
//!
//! The emitted Ruby is parsed and executed; these controls survive the
//! migration from the fork's earlier conditional-ladder representation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

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

fn assert_execution(out: &str, assertions: &str) {
    let run = Command::new("ruby")
        .args(["-e", &format!("{out}\n{assertions}")])
        .output()
        .expect("run emitted Ruby");
    assert!(run.status.success(), "{}\n{out}", String::from_utf8_lossy(&run.stderr));
}

#[test]
fn an_array_pattern_with_alternatives_dispatches_natively() {
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
    assert_execution(&out, r#"
class Ad; end
class AdGroup; end
class Campaign; end
m = Matcher.new
raise unless m.check(Ad.new, nil) == 1
raise unless m.check(Ad.new, Ad.new) == 1
raise unless m.check(AdGroup.new, Campaign.new) == 2
raise unless m.check(nil, nil) == 3
"#);
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
    assert_execution(&out, r#"
raise unless Matcher.new.check(3) == 1
begin
  Matcher.new.check("miss")
  raise "a miss did not raise"
rescue NoMatchingPatternError
end
"#);
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
    assert_execution(&out, "raise unless Matcher.new.check([3, 'name']) == [3, 'name']\nraise unless Matcher.new.check(['miss', 'name']).nil?");
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
    assert_execution(&out, "raise unless Matcher.new.check([1, 2, 3, 4]) == [2, 3]\nraise unless Matcher.new.check([1, 4]) == []\nraise unless Matcher.new.check([1]).nil?");
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
    assert_execution(&out, "raise unless Matcher.new.check({type: :card, amount: 8, extra: 9}) == 8\nraise unless Matcher.new.check({type: :card}) == 0\nraise unless Matcher.new.check({type: :cash, amount: 8}) == 0");
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
    assert_execution(&out, "raise unless Matcher.new.check([2]) == :big\nraise unless Matcher.new.check([1]) == :small\nraise unless Matcher.new.check([]) == :none");
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
    assert_execution(&out, "raise unless Matcher.new.check(7, 7) == 1\nraise unless Matcher.new.check(7, 8) == 2");
}
