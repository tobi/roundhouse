//! Compile-time class-body declarations (#30): enum, attribute,
//! literal `define_method`, interpolatable `class_eval` heredocs, and
//! the same expansion when the class-method provider is an
//! `on_load(:active_record)` include rather than an `include` on the
//! model.
//!
//! Forcing coverage is synthetic overlays on the blog fixtures — not a
//! named app macro. Dynamic string eval stays unexpanded.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::dialect::{Association, MethodReceiver, ModelBodyItem};
use roundhouse::expr::ExprNode;
use roundhouse::ingest::ingest_app_from_tree;

#[path = "support/emit_and_run.rs"]
mod emit_and_run;

const HEREDOC_MACRO: &str = r#"module TitleMacro
  extend ActiveSupport::Concern
  class_methods do
    def titled(name)
      class_eval <<-CODE, __FILE__, __LINE__ + 1
        def #{name}
          @#{name}.to_s
        end
        def #{name}=(value)
          @#{name} = value
        end
      CODE
    end
  end
end
"#;

const CHILD_MACRO: &str = r#"module ChildMacro
  extend ActiveSupport::Concern
  class_methods do
    def child_named(name)
      has_one name, class_name: "Comment", foreign_key: :article_id
      class_eval <<-CODE, __FILE__, __LINE__ + 1
        def #{name}_present?
          !#{name}.nil?
        end
      CODE
    end
  end
end
"#;

fn tree(files: &[(&str, &str)]) -> HashMap<PathBuf, Vec<u8>> {
    files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect()
}

fn article_with_macro(
    concern_path: &str,
    concern: &str,
    include_name: &str,
    model_body: &str,
) -> HashMap<PathBuf, Vec<u8>> {
    tree(&[
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table :articles do |t|\n    t.string :title\n  end\n  create_table :comments do |t|\n    t.text :body\n    t.bigint :article_id\n  end\nend\n",
        ),
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n",
        ),
        (concern_path, concern),
        (
            "app/models/article.rb",
            &format!(
                "class Article < ApplicationRecord\n  include {include_name}\n  {model_body}\nend\n"
            ),
        ),
        (
            "app/models/comment.rb",
            "class Comment < ApplicationRecord\n  belongs_to :article\nend\n",
        ),
    ])
}

fn instance_names(app: &roundhouse::App, model: &str) -> Vec<String> {
    app.models
        .iter()
        .find(|m| m.name.0.as_str() == model)
        .expect(model)
        .methods()
        .filter(|m| m.receiver == MethodReceiver::Instance)
        .map(|m| m.name.as_str().to_string())
        .collect()
}

#[test]
fn heredoc_class_eval_expands_interpolated_accessors() {
    let app = ingest_app_from_tree(article_with_macro(
        "app/models/concerns/title_macro.rb",
        HEREDOC_MACRO,
        "TitleMacro",
        "titled :headline",
    ))
    .expect("ingest");
    let names = instance_names(&app, "Article");
    assert!(names.contains(&"headline".to_string()), "{names:?}");
    assert!(names.contains(&"headline=".to_string()), "{names:?}");
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(!article.body.iter().any(|item| matches!(
        item,
        ModelBodyItem::Unknown { expr, .. }
            if matches!(&*expr.node, ExprNode::Send { method, .. } if method.as_str() == "titled")
    )));
}

#[test]
fn class_eval_macro_also_ingests_substituted_has_one() {
    let app = ingest_app_from_tree(article_with_macro(
        "app/models/concerns/child_macro.rb",
        CHILD_MACRO,
        "ChildMacro",
        "child_named :spotlight",
    ))
    .expect("ingest");
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(article.body.iter().any(|item| matches!(
        item,
        ModelBodyItem::Association {
            assoc: Association::HasOne { name, .. },
            ..
        } if name.as_str() == "spotlight"
    )));
    let names = instance_names(&app, "Article");
    assert!(
        names.contains(&"spotlight_present?".to_string()),
        "{names:?}"
    );
}

#[test]
fn load_hook_class_macros_expand_into_model_methods() {
    let files = tree(&[
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table :articles do |t|\n    t.string :title\n  end\nend\n",
        ),
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n",
        ),
        (
            "lib/title_macro.rb",
            &format!(
                "{HEREDOC_MACRO}\nActiveSupport.on_load :active_record do\n  include TitleMacro\nend\n"
            ),
        ),
        (
            "app/models/article.rb",
            "class Article < ApplicationRecord\n  titled :headline\nend\n",
        ),
    ]);
    roundhouse::ingest::survey::activate();
    let app = ingest_app_from_tree(files).expect("survey ingest");
    roundhouse::ingest::survey::drain();
    let names = instance_names(&app, "Article");
    assert!(names.contains(&"headline".to_string()), "{names:?}");
    assert!(names.contains(&"headline=".to_string()), "{names:?}");
    assert!(!names
        .iter()
        .any(|n| n.contains("installer") || n == "titled"));
}

#[test]
fn dynamic_class_eval_string_is_not_expanded() {
    let concern = r#"module TitleMacro
  extend ActiveSupport::Concern
  class_methods do
    def titled(name)
      template = "def #{name}; 99; end"
      class_eval template
    end
  end
end
"#;
    let files = tree(&[
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table :articles do |t|\n    t.string :title\n  end\nend\n",
        ),
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n",
        ),
        ("app/models/concerns/title_macro.rb", concern),
        (
            "app/models/article.rb",
            "class Article < ApplicationRecord\n  include TitleMacro\n  titled :headline\nend\n",
        ),
    ]);
    roundhouse::ingest::survey::activate();
    let app = ingest_app_from_tree(files).expect("survey ingest");
    roundhouse::ingest::survey::drain();
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(article.body.iter().any(|item| matches!(
        item,
        ModelBodyItem::Unknown { expr, .. }
            if matches!(&*expr.node, ExprNode::Send { method, .. } if method.as_str() == "titled")
    )));
    assert!(!article
        .methods()
        .any(|m| m.name.as_str() == "headline" && m.receiver == MethodReceiver::Instance));
}

#[test]
fn tiny_blog_enum_and_attribute_ingest() {
    let files = tree(&[
        (
            "db/schema.rb",
            include_str!("../fixtures/tiny-blog/db/schema.rb"),
        ),
        (
            "app/models/post.rb",
            r#"class Post < ApplicationRecord
  has_many :comments
  validates :title, presence: true
  scope :recent, -> { limit(10) }
  scope :published, -> { where(published: true) }
  before_save :normalize_title
  enum :status, %i[draft published], default: :draft
  attribute :flagged, :boolean

  def normalize_title
    title.strip
  end
end
"#,
        ),
        (
            "app/models/comment.rb",
            include_str!("../fixtures/tiny-blog/app/models/comment.rb"),
        ),
    ]);
    let app = ingest_app_from_tree(files).expect("ingest");
    let names = instance_names(&app, "Post");
    assert!(names.iter().any(|n| n == "draft?"), "{names:?}");
    assert!(names.iter().any(|n| n == "published?"), "{names:?}");
    let post = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Post")
        .unwrap();
    let lowered = roundhouse::lower::lower_model_to_library_class(post, &app.schema);
    assert!(
        lowered.methods.iter().any(|m| m.name.as_str() == "flagged"),
        "attribute reader"
    );
    assert!(
        lowered
            .methods
            .iter()
            .any(|m| m.name.as_str() == "flagged="),
        "attribute writer"
    );
}

#[test]
fn emitted_heredoc_class_eval_accessors_run() {
    emit_and_run::real_blog()
        .write("app/models/concerns/title_macro.rb", HEREDOC_MACRO)
        .edit(
            "app/models/article.rb",
            "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  include TitleMacro\n  titled :headline\n",
        )
        .run_ruby(
            r#"
a = Article.new
a.headline = "Hello"
raise "writer lost" unless a.headline == "Hello"
puts "class_eval heredoc accessors passed"
"#,
        )
        .assert_passes();
}

#[test]
fn emitted_load_hook_class_eval_accessors_and_literal_mixin_run() {
    emit_and_run::real_blog()
        .write(
            "lib/title_macro.rb",
            r#"module TitleMacro
  extend ActiveSupport::Concern
  class_methods do
    def titled(name)
      class_eval <<-CODE, __FILE__, __LINE__ + 1
        def #{name}
          @#{name}.to_s
        end
        def #{name}=(value)
          @#{name} = value
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
"#,
        )
        .edit(
            "app/models/article.rb",
            "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  titled :headline\n",
        )
        .run_ruby(
            r#"
a = Article.new
a.headline = "Hello"
raise "writer lost" unless a.headline == "Hello"
raise "hook lost instance method" unless a.installer_marker == 37
puts "load-hook class_eval accessors passed"
"#,
        )
        .assert_passes();
}

#[test]
fn emitted_enum_and_boolean_attribute_run() {
    emit_and_run::real_blog()
        .edit(
            "app/models/article.rb",
            "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  enum :status, %i[draft published], default: :draft\n  attribute :flagged, :boolean\n",
        )
        .edit(
            "db/schema.rb",
            "    t.string \"title\"",
            "    t.string \"title\"\n    t.string \"status\"",
        )
        .run_ruby(
            r#"
a = Article.new(title: "Hello world", body: "abcdefghij")
raise "enum default" unless a.draft?
a.published!
raise "enum bang" unless a.published?
a.flagged = "0"
raise "boolean attribute" unless a.flagged == false
a.flagged = "1"
raise "boolean attribute true" unless a.flagged == true
puts "enum and attribute passed"
"#,
        )
        .assert_passes();
}

#[test]
fn emitted_literal_define_method_macro_runs() {
    emit_and_run::real_blog()
        .write(
            "app/models/concerns/label_macro.rb",
            r#"module LabelMacro
  extend ActiveSupport::Concern
  class_methods do
    def labeled(field)
      define_method :label_of do
        send(field)
      end
    end
  end
end
"#,
        )
        .edit(
            "app/models/article.rb",
            "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  include LabelMacro\n  labeled :title\n",
        )
        .run_ruby(
            r#"
a = Article.new(title: "Hello world", body: "abcdefghij")
raise "define_method" unless a.label_of == "Hello world"
puts "literal define_method passed"
"#,
        )
        .assert_passes();
}

#[test]
fn emitted_substituted_has_one_and_class_eval_predicate_run() {
    emit_and_run::real_blog()
        .write("app/models/concerns/child_macro.rb", CHILD_MACRO)
        .edit(
            "app/models/article.rb",
            "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  include ChildMacro\n  child_named :spotlight\n",
        )
        .run_ruby(
            r#"
a = Article.create!(title: "Hello world", body: "abcdefghij")
raise "empty has_one" unless a.spotlight_present? == false
Comment.create!(article: a, body: "hi", commenter: "x")
raise "has_one reader" unless a.spotlight_present?
raise "wrong child" unless a.spotlight.body == "hi"
puts "substituted has_one passed"
"#,
        )
        .assert_passes();
}

#[test]
fn missing_required_keyword_does_not_expand() {
    let concern = r#"module LabelMacro
  extend ActiveSupport::Concern
  class_methods do
    def labeled(title:)
      define_method title do
        1
      end
    end
  end
end
"#;
    roundhouse::ingest::survey::activate();
    let app = ingest_app_from_tree(article_with_macro(
        "app/models/concerns/label_macro.rb",
        concern,
        "LabelMacro",
        "labeled",
    ))
    .expect("survey ingest");
    roundhouse::ingest::survey::drain();
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(
        article.body.iter().any(|item| matches!(
            item,
            ModelBodyItem::Unknown { expr, .. }
                if matches!(&*expr.node, ExprNode::Send { method, .. } if method.as_str() == "labeled")
        )),
        "required keyword missing must fail closed"
    );
    let names = instance_names(&app, "Article");
    assert!(
        !names.iter().any(|n| n == "headline"),
        "must not invent a helper: {names:?}"
    );
}

const LEFTOVER_MACRO: &str = r##"module AttachMacro
  extend ActiveSupport::Concern
  class_methods do
    def attached(name, strict_loading: false)
      has_one :"#{name}_record", class_name: "Comment", foreign_key: :article_id, strict_loading: strict_loading
      scope :"with_#{name}", -> { all }
      class_eval <<-CODE, __FILE__, __LINE__ + 1
        def #{name}
          1
        end
      CODE
    end
  end
end
"##;

/// Interpolatable class_eval plus leftover interpolated association /
/// scope names must fail closed: do not emit the class_eval methods
/// when the rest of the macro is not ingestible.
#[test]
fn leftover_interpolated_association_does_not_expand() {
    roundhouse::ingest::survey::activate();
    let app = ingest_app_from_tree(article_with_macro(
        "app/models/concerns/attach_macro.rb",
        LEFTOVER_MACRO,
        "AttachMacro",
        "attached :spotlight",
    ))
    .expect("survey ingest");
    roundhouse::ingest::survey::drain();
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(
        article.body.iter().any(|item| matches!(
            item,
            ModelBodyItem::Unknown { expr, .. }
                if matches!(&*expr.node, ExprNode::Send { method, .. } if method.as_str() == "attached")
        )),
        "call must stay unexpanded"
    );
    let names = instance_names(&app, "Article");
    assert!(
        !names.iter().any(|n| n == "spotlight" || n == "spotlight_record"),
        "must not invent readers: {names:?}"
    );
}

/// Same leftover shape through `on_load(:active_record)` — the Writebook
/// installer path — without claiming a named-app runtime.
#[test]
fn load_hook_leftover_interpolated_association_does_not_expand() {
    roundhouse::ingest::survey::activate();
    let app = ingest_app_from_tree(tree(&[
        (
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table :articles do |t|\n    t.string :title\n  end\n  create_table :comments do |t|\n    t.text :body\n    t.bigint :article_id\n  end\nend\n",
        ),
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n",
        ),
        (
            "lib/attach_macro.rb",
            &format!(
                "{LEFTOVER_MACRO}\nActiveSupport.on_load :active_record do\n  include AttachMacro\nend\n"
            ),
        ),
        (
            "app/models/article.rb",
            "class Article < ApplicationRecord\n  attached :spotlight\nend\n",
        ),
        (
            "app/models/comment.rb",
            "class Comment < ApplicationRecord\n  belongs_to :article\nend\n",
        ),
    ]))
    .expect("survey ingest");
    roundhouse::ingest::survey::drain();
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(
        article.body.iter().any(|item| matches!(
            item,
            ModelBodyItem::Unknown { expr, .. }
                if matches!(&*expr.node, ExprNode::Send { method, .. } if method.as_str() == "attached")
        )),
        "load-hook call must stay unexpanded"
    );
    let names = instance_names(&app, "Article");
    assert!(
        !names.iter().any(|n| n == "spotlight" || n == "spotlight_record"),
        "must not invent readers: {names:?}"
    );
}

/// Optional non-symbol kwargs are omitted from the binding set. A
/// leftover `strict_loading: strict_loading` must decline the whole
/// expansion rather than ingest `has_one` without that option.
#[test]
fn omitted_optional_keyword_read_does_not_expand() {
    let concern = r#"module TitleMacro
  extend ActiveSupport::Concern
  class_methods do
    def titled(name, strict_loading: false)
      has_one :spotlight, class_name: "Comment", foreign_key: :article_id, strict_loading: strict_loading
      class_eval <<-CODE, __FILE__, __LINE__ + 1
        def #{name}
          1
        end
      CODE
    end
  end
end
"#;
    roundhouse::ingest::survey::activate();
    let app = ingest_app_from_tree(article_with_macro(
        "app/models/concerns/title_macro.rb",
        concern,
        "TitleMacro",
        "titled :headline",
    ))
    .expect("survey ingest");
    roundhouse::ingest::survey::drain();
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(
        article.body.iter().any(|item| matches!(
            item,
            ModelBodyItem::Unknown { expr, .. }
                if matches!(&*expr.node, ExprNode::Send { method, .. } if method.as_str() == "titled")
        )),
        "omitted keyword read must stay unexpanded"
    );
    let names = instance_names(&app, "Article");
    assert!(
        !names.iter().any(|n| n == "headline" || n == "spotlight"),
        "must not invent readers: {names:?}"
    );
}

/// A slice-local that shadows a macro parameter (`->(name) { … name }`)
/// is not the bound symbol. Decline rather than rewrite Ruby's lambda
/// argument into `:headline`.
#[test]
fn shadowed_macro_parameter_does_not_expand() {
    let concern = r#"module TitleMacro
  extend ActiveSupport::Concern
  class_methods do
    def titled(name)
      scope :named, ->(name) { where(title: name) }
      class_eval <<-CODE, __FILE__, __LINE__ + 1
        def #{name}
          1
        end
      CODE
    end
  end
end
"#;
    roundhouse::ingest::survey::activate();
    let app = ingest_app_from_tree(article_with_macro(
        "app/models/concerns/title_macro.rb",
        concern,
        "TitleMacro",
        "titled :headline",
    ))
    .expect("survey ingest");
    roundhouse::ingest::survey::drain();
    let article = app
        .models
        .iter()
        .find(|m| m.name.0.as_str() == "Article")
        .unwrap();
    assert!(
        article.body.iter().any(|item| matches!(
            item,
            ModelBodyItem::Unknown { expr, .. }
                if matches!(&*expr.node, ExprNode::Send { method, .. } if method.as_str() == "titled")
        )),
        "shadowed parameter must stay unexpanded"
    );
    let names = instance_names(&app, "Article");
    assert!(
        !names.iter().any(|n| n == "headline"),
        "must not invent readers: {names:?}"
    );
}

/// Strict ingest keeps the abort as an error — survey records a gap,
/// emit-and-run would panic, and we must not drop the diagnostic to
/// emit a reader without the association.
#[test]
fn leftover_interpolated_association_stays_an_ingest_error() {
    let err = ingest_app_from_tree(article_with_macro(
        "app/models/concerns/attach_macro.rb",
        LEFTOVER_MACRO,
        "AttachMacro",
        "attached :spotlight",
    ))
    .expect_err("strict ingest must fail closed");
    let message = err.to_string();
    assert!(
        message.contains("model macro `attached` not expanded"),
        "{message}"
    );
}

