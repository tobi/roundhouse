#[path = "support/emit_and_run.rs"]
mod emit_and_run;

#[test]
fn duplicate_enum_values_read_the_first_declared_label() {
    emit_and_run::real_blog()
        .edit("db/schema.rb", "create_table \"articles\", force: :cascade do |t|",
            "create_table \"articles\", force: :cascade do |t|\n    t.integer \"stage\"\n    t.string \"tone\"")
        .edit("app/models/article.rb", "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  enum :stage, { early: 1, late: 1, other: 3 }\n  enum :tone, { soft: \"s\", gentle: \"s\", loud: \"l\" }, prefix: true\n")
        .run_ruby(r#"
article = Article.create!(title: "Aliases", body: "A body long enough to validate.", stage: :late, tone: :gentle)
reloaded = Article.find(article.id)
raise "integer alias" unless reloaded.stage == "early"
raise "string alias" unless reloaded.tone == "soft"
raise "index aliases" unless reloaded[:stage] == "early" && reloaded[:tone] == "soft"
raise "attribute aliases" unless reloaded.attributes["stage"] == "early" && reloaded.attributes["tone"] == "soft"
raise "integer predicate" unless reloaded.early? && reloaded.late? && !reloaded.other?
raise "string predicate" unless reloaded.tone_soft? && reloaded.tone_gentle? && !reloaded.tone_loud?
raise "nil enum" unless Article.new.stage.nil? && Article.new.tone.nil?
reloaded.update!(stage: :other, tone: :loud)
raise "distinct values" unless reloaded.reload.stage == "other" && reloaded.tone == "loud"
"#).assert_passes();
}

#[test]
fn nullable_string_inflections_raise_and_evaluate_the_receiver_once() {
    emit_and_run::real_blog()
        .write("app/services/inflection_probe.rb", r#"class InflectionProbe
  attr_reader :reads
  def initialize
    @reads = 0
  end
  def value
    @reads += 1
    @reads == 1 ? "author_id" : nil
  end
  def label
    value.humanize
  end
  def heading
    value.titleize
  end
  def optional_label
    current = value
    current&.humanize
  end
end
"#)
        .run_ruby(r#"
[:label, :heading].each do |method|
  probe = InflectionProbe.new
  raise "string inflection" unless probe.public_send(method) == "Author"
  raised = false
  begin
    probe.public_send(method)
  rescue NoMethodError
    raised = true
  end
  raise "nil must raise #{method}" unless raised
  raise "receiver evaluated repeatedly" unless probe.reads == 2
end
probe = InflectionProbe.new
raise "safe-navigation string" unless probe.optional_label == "Author"
raise "safe-navigation nil" unless probe.optional_label.nil?
raise "safe-navigation reads" unless probe.reads == 2
"#).assert_passes();
}
