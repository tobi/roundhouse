//! Rails preserves original enum assignment inputs before persistence.
#[path = "support/emit_and_run.rs"]
mod emit_and_run;

#[test]
fn enum_before_type_cast_remains_unsupported_until_input_provenance_is_modeled() {
    let run = emit_and_run::real_blog()
        .edit("db/schema.rb", "create_table \"articles\", force: :cascade do |t|",
            "create_table \"articles\", force: :cascade do |t|\n    t.integer \"state\"")
        .edit("app/models/article.rb", "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  enum :state, { draft: 0, published: 1 }\n  def raw_state\n    state_before_type_cast\n  end\n")
        .run_ruby(r#"
article = Article.new(state: :published)
raise "lost original enum input: #{article.raw_state.inspect}" unless article.raw_state == :published
"#);
    assert!(run.errors.iter().any(|error| error.contains("state_before_type_cast")), "raw input must not be claimed as supported: {:?}", run.errors);
}

#[test]
fn source_defined_before_type_cast_methods_keep_their_behavior() {
    emit_and_run::real_blog()
        .edit("db/schema.rb", "create_table \"articles\", force: :cascade do |t|",
            "create_table \"articles\", force: :cascade do |t|\n    t.integer \"state\"")
        .write("app/models/concerns/raw_state.rb", "module RawState\n  def state_before_type_cast\n    \"custom\"\n  end\nend\n")
        .edit("app/models/article.rb", "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  enum :state, { draft: 0, published: 1 }\n  include RawState\n  def raw_state\n    state_before_type_cast\n  end\n")
        .run_ruby("raise 'custom raw state' unless Article.new.raw_state == 'custom'")
        .assert_passes();
}
