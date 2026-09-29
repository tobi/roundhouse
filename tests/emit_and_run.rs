//! Constructs that `check` accepts must run once emitted.
//!
//! See `tests/support/emit_and_run.rs` for the harness and why it
//! exists. The ignored tests below are known places where the two
//! disagree: `check` is clean and the emitted program fails. Each is a
//! complete statement of the fix: make it pass and drop the `#[ignore]`.

#[path = "support/emit_and_run.rs"]
mod emit_and_run;

/// The harness itself: the unedited blog emits and its controller
/// suite, which renders every page, passes.
#[test]
fn the_unedited_blog_runs() {
    emit_and_run::real_blog()
        .run_test("test/controllers/articles_controller_test.rb")
        .assert_passes();
}

/// Unlike a hash pattern with keys, `{}` requires the hash to be empty.
/// A bare `is_a?(Hash)` check silently chose the wrong case arm for
/// every nonempty hash. `**` explicitly permits the remaining keys.
#[test]
fn an_empty_hash_pattern_rejects_extra_keys() {
    emit_and_run::real_blog()
        .write(
            "app/helpers/hash_pattern_probe.rb",
            r#"class HashPatternProbe
  #: (Hash[Symbol, Integer]) -> bool
  def self.empty_match(value)
    case value
    in {}
      true
    else
      false
    end
  end

  #: (Hash[Symbol, Integer]) -> bool
  def self.open_match(value)
    case value
    in { ** }
      true
    else
      false
    end
  end

  #: (Hash[Symbol, Integer]) -> bool
  def self.key_match(value)
    case value
    in { x: 1 }
      true
    else
      false
    end
  end
end
"#,
        )
        .run_ruby(
            r#"raise "empty hash did not match" unless HashPatternProbe.empty_match({})
raise "nonempty hash incorrectly matched {}" if HashPatternProbe.empty_match({ x: 1 })
raise "open hash pattern rejected extra keys" unless HashPatternProbe.open_match({ x: 1 })
raise "keyed pattern rejected extra keys" unless HashPatternProbe.key_match({ x: 1, y: 2 })
raise "keyed pattern accepted a missing key" if HashPatternProbe.key_match({ y: 2 })
"#,
        )
        .assert_passes();
}

/// #139 typed `Model.human_attribute_name` as a String, which took the
/// call from an error to clean, but no runtime defines it, so every
/// page rendering the form raises `undefined method
/// 'human_attribute_name' for class Article`. It belongs once, in
/// `runtime/ruby/active_record/base.rb`, where every target gets it.
#[test]
#[ignore = "check is clean but the emitted view raises NoMethodError: no runtime defines human_attribute_name (#147)"]
fn human_attribute_name_runs() {
    emit_and_run::real_blog()
        .edit(
            "app/views/articles/_form.html.erb",
            "<%= form.label :title %>",
            "<%= form.label :title %><%= Article.human_attribute_name(:title) %>",
        )
        .run_test("test/controllers/articles_controller_test.rb")
        .assert_passes();
}

/// #140 bound `form_with builder: X`'s block param to `X`, which took a
/// custom builder's own helpers from errors to clean. But the emitted
/// tree cannot load `X` (no runtime `ActionView::Helpers::FormBuilder`
/// to subclass), and the view calls `form.marker_field` on a `form`
/// that no longer exists, because lowering expands the stock builder
/// inline. Passing needs a builder the emitted view can call; until
/// then, the honest state is an error in `check`.
#[test]
#[ignore = "check is clean but the emitted tree fails to load: no runtime FormBuilder, and the inlined form has no builder object (#148)"]
fn a_custom_form_builder_runs() {
    emit_and_run::real_blog()
        .write(
            "app/helpers/custom_form_builder.rb",
            "class CustomFormBuilder < ActionView::Helpers::FormBuilder\n  \
               def marker_field(name)\n    \
                 @template.content_tag(:span, name.to_s, class: \"builder-marker\")\n  \
               end\n\
             end\n",
        )
        .edit(
            "app/views/articles/_form.html.erb",
            "form_with(model: article, class: \"contents\")",
            "form_with(model: article, class: \"contents\", builder: CustomFormBuilder)",
        )
        .edit(
            "app/views/articles/_form.html.erb",
            "<%= form.label :title %>",
            "<%= form.label :title %><%= form.marker_field :title %>",
        )
        .run_test("test/controllers/articles_controller_test.rb")
        .assert_passes();
}
