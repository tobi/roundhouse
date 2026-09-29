//! A class can define `def self.to_entity(record, lookup: nil)` beside
//! `def to_entity(lookup: nil)`. The signature table has one entry per
//! (class, name) and both sides read it, so the last signature read
//! typed both bodies: the class method's `record` came out as the
//! instance method's keyword `lookup` (`Hash?`), and every call on it
//! was a dispatch on that.
//!
//! A signature that would apply to the wrong side is dropped instead;
//! the method is inferred as before.

use roundhouse::ident::{ClassId, Symbol};
use roundhouse::ingest::sorbet_sig::ingest_sorbet_signatures;

fn declared(source: &str, class: &str) -> Vec<String> {
    let mut names: Vec<String> = ingest_sorbet_signatures(source.as_bytes())
        .remove(&ClassId(Symbol::new(class)))
        .unwrap_or_default()
        .keys()
        .map(|k| k.as_str().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn a_name_defined_on_both_sides_keeps_neither_signature() {
    let source = r#"class Entity
  class << self
    #: (Record, ?lookup: Hash[Integer, Integer]?) -> Integer
    def to_entity(record, lookup: nil); 1; end

    #: (Record) -> Integer
    def only_class(record); 1; end
  end

  #: (?lookup: Hash[Integer, Integer]?) -> Integer
  def to_entity(lookup: nil); 1; end

  #: (Record) -> Integer
  def only_instance(record); 1; end
end
"#;
    assert_eq!(declared(source, "Entity"), vec!["only_class", "only_instance"]);
}

#[test]
fn a_sorbet_sig_and_a_self_def_of_the_same_name_are_ambiguous_too() {
    let source = r#"class Entity
  sig { params(record: Record).returns(Integer) }
  def self.build(record); 1; end

  sig { params(lookup: T.nilable(Integer)).returns(Integer) }
  def build(lookup = nil); 1; end
end
"#;
    assert_eq!(declared(source, "Entity"), Vec::<String>::new());
}
