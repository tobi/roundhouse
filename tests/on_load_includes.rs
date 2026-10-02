//! Known literal load-hook mixins are carried; unsupported hooks and
//! dynamic includes remain reported gaps. This does not model Markdown DSLs.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::dialect::ModelBodyItem;
use roundhouse::expr::ExprNode;
use roundhouse::ingest::{ingest_app_from_tree, survey};

#[path = "support/emit_and_run.rs"]
mod emit_and_run;

fn tree(hooks: &str) -> HashMap<PathBuf, Vec<u8>> {
    [
        ("config/routes.rb", "Rails.application.routes.draw do\nend\n"),
        ("lib/rails_ext/hooks.rb", hooks),
        (
            "app/models/article.rb",
            "class Article < ApplicationRecord\n  labeled :headline\nend\n",
        ),
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table :articles do |t|\n    t.string :title\n  end\nend\n",
        ),
    ]
    .into_iter()
    .map(|(p, s)| (PathBuf::from(p), s.as_bytes().to_vec()))
    .collect()
}

#[test]
fn direct_includes_report_only_uncarried_hook_shapes() {
    // First hook is Writebook's installer, verbatim. Later hooks prove
    // that a literal-name-only recognizer cannot silently skip other shapes.
    let files = tree(
        r#"ActiveSupport.on_load :active_record do
  include LabelMacro
end
ActiveSupport.on_load(:action_text_markdown) do
  include First, Second
  include choose_mixin("märgistus")
end
"#,
    );
    let mut strict = ingest_app_from_tree(files.clone()).expect("strict ingest");
    survey::activate();
    let result = ingest_app_from_tree(files);
    let gaps = survey::drain();
    let mut surveyed = result.expect("survey ingest");
    let messages: Vec<_> = gaps.iter().map(ToString::to_string).collect();
    assert_eq!(messages.len(), 2, "one gap per uncarried include: {messages:?}");
    for (hook, include) in [
        ("action_text_markdown", "include First, Second"),
        (
            "action_text_markdown",
            "include choose_mixin(\"märgistus\")",
        ),
    ] {
        assert_eq!(
            messages
                .iter()
                .filter(|m| m.contains("lib/rails_ext/hooks.rb")
                    && m.contains(&format!("on_load(:{hook})"))
                    && m.contains(&format!("`{include}`"))
                    && m.contains("load-hook mixin installation is unsupported"))
                .count(),
            1
        );
    }
    assert_eq!(surveyed, strict, "reporting must not change ingested IR");
    assert!(surveyed.library_classes.iter().any(|class|
        class.name.0.as_str() == "ActiveRecord::Base"
            && class.includes.iter().any(|id| id.0.as_str() == "LabelMacro")));
    let article = surveyed
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(article.body.iter().any(|item| matches!(item,
        ModelBodyItem::Unknown { expr, .. } if matches!(&*expr.node,
            ExprNode::Send { method, .. } if method.as_str() == "labeled"))));
    let strict_diags = roundhouse::session::analyze_and_lower(&mut strict);
    let survey_diags = roundhouse::session::analyze_and_lower(&mut surveyed);
    assert_eq!(
        survey_diags, strict_diags,
        "no analysis/lowering support added"
    );
    let (strict_files, strict_emit) = roundhouse::emit::diagnostics::scope(|| {
        roundhouse::emit::ruby::emit_lowered_models(&strict)
    });
    let (survey_files, survey_emit) = roundhouse::emit::diagnostics::scope(|| {
        roundhouse::emit::ruby::emit_lowered_models(&surveyed)
    });
    assert_eq!(survey_files, strict_files, "no emitted behavior changed");
    assert_eq!(survey_emit, strict_emit);
    assert!(
        survey_emit.iter().any(|d| d.message.contains("labeled")),
        "declaration warning must remain: {survey_emit:?}"
    );
    let article = survey_files
        .iter()
        .find(|f| f.path.ends_with("article.rb"))
        .unwrap();
    for method in ["headline", "headline?", "headline=", "build_headline", "with_headline"] {
        assert!(
            !article.content.lines().any(|line| line
                .trim_start()
                .starts_with(&format!("def {method}("))
                || line.trim() == format!("def {method}")),
            "not supported: {method}\n{}",
            article.content
        );
    }
}

#[test]
fn only_direct_receiverless_includes_in_top_level_active_support_hooks_are_reported() {
    survey::activate();
    let result = ingest_app_from_tree(tree(
        r#"ActiveSupport.on_load(:active_record) do
  self.include ExplicitReceiver
  Other.include OtherReceiver
  if enabled?
    include Conditional
  end
  def deferred
    include MethodBody
  end
  class Reopened
    include ClassBody
    def marker; 1; end
  end
end
Other.on_load(:active_record) do
  include NotActiveSupport
end
register do
  ActiveSupport.on_load(:active_record) do
    include NestedHook
  end
end
include NotInHook
"#,
    ));
    let gaps = survey::drain();
    result.expect("ingest controls");
    let messages: Vec<_> = gaps.iter().map(ToString::to_string).collect();
    assert_eq!(messages.len(), 1, "only existing reopen gap: {messages:?}");
    assert!(messages[0].contains("reopens `Reopened`"));
    assert!(!messages[0].contains("mixin installation"));
}

#[test]
fn binary_encoded_source_uses_original_byte_locations_before_lossy_display() {
    let mut files = tree("");
    // An invalid UTF-8 byte before the include shifts lossy-string
    // offsets; one inside the argument can put an endpoint inside U+FFFD.
    files.insert(PathBuf::from("lib/rails_ext/hooks.rb"),
        b"# encoding: ASCII-8BIT\n# \xff\nActiveSupport.on_load(:active_record) do\n  include First\n  include \"\xff\"\nend\n".to_vec());
    let strict = ingest_app_from_tree(files.clone()).expect("strict binary-source ingest");
    survey::activate();
    let result = ingest_app_from_tree(files);
    let gaps = survey::drain();
    let surveyed = result.expect("survey binary-source ingest");
    assert_eq!(surveyed, strict, "a ledger cannot change binary-source IR");
    let messages: Vec<_> = gaps.iter().map(ToString::to_string).collect();
    assert_eq!(messages.len(), 1, "{messages:?}");
    for (message, declaration) in messages.iter().zip(["include \"�\""]) {
        assert!(message.contains(&format!("`{declaration}`")), "{message}");
        assert!(message.contains("on_load(:active_record)"), "{message}");
        assert!(message.contains("lib/rails_ext/hooks.rb"), "{message}");
    }
}

#[test]
fn emitted_app_carries_literal_hook_methods_and_expands_its_class_macro() {
    emit_and_run::real_blog()
        .write("lib/rails_ext/title_macro.rb", r#"module TitleMacro
  extend ActiveSupport::Concern
  class_methods do
    def titled(name)
      class_eval <<-CODE, __FILE__, __LINE__ + 1
        def #{name}
          title
        end
      CODE
    end
  end
  def installer_marker
    37
  end
end
ActiveSupport.on_load :active_record do
  include TitleMacro
end
"#)
        .edit("app/models/article.rb", "class Article < ApplicationRecord", "class Article < ApplicationRecord\n  titled :headline")
        .run_ruby(r#"
a = Article.new(title: "hello")
raise "literal hook lost mixin" unless a.installer_marker == 37
raise "hook macro did not expand" unless a.headline == "hello"
"#)
        .assert_passes();
}
