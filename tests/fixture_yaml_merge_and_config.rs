//! Fixture YAML the way Psych and `ActiveRecord::FixtureSet::File` read
//! it: merge keys resolved, the `_fixture:` configuration row honoured,
//! and `test/fixtures/files/` left out.
//!
//! Shopify core is the forcing function: 131 of its fixture files build
//! rows by merging an anchored one (often in chains three or four deep),
//! and 1,532 open with `_fixture:` naming the model class and the
//! template rows to ignore. Before, a merged row failed as "fixture
//! field x.<< is not a scalar", and `_fixture` loaded as a row of its own.

use std::path::{Path, PathBuf};

use roundhouse::dialect::{Fixture, FixtureValue};
use roundhouse::ingest::{ingest_fixture_file, IngestError};
use roundhouse::Symbol;

fn fixture(yml: &str) -> Result<Fixture, IngestError> {
    ingest_fixture_file(
        yml.as_bytes(),
        Path::new("test/fixtures/things.yml"),
        Path::new("test/fixtures"),
    )
}

fn field(f: &Fixture, label: &str, key: &str) -> Option<String> {
    match f.records.get(&Symbol::from(label))?.get(&Symbol::from(key))? {
        FixtureValue::Scalar(s) => Some(s.clone()),
        FixtureValue::Ruby(_) => None,
    }
}

/// `c` merges `b`, which merges `a`. The copy of `b` under `c`'s `<<`
/// still has its own `<<`, so the innermost merge has to resolve first.
#[test]
fn a_merge_chain_resolves_innermost_first() {
    let f = fixture(
        "a: &a\n  name: A\n  color: red\n\
         b: &b\n  <<: *a\n  name: B\n  size: big\n\
         c:\n  <<: *b\n  name: C\n",
    )
    .expect("ingest");
    assert_eq!(field(&f, "c", "name").as_deref(), Some("C"));
    assert_eq!(field(&f, "c", "size").as_deref(), Some("big"));
    assert_eq!(field(&f, "c", "color").as_deref(), Some("red"));
    assert_eq!(field(&f, "b", "name").as_deref(), Some("B"));
    assert!(f.records.values().all(|r| !r.contains_key(&Symbol::from("<<"))));
}

/// `<<: [*a, *b]`: the earlier mapping wins a key both set.
#[test]
fn in_a_merge_list_the_earlier_mapping_wins() {
    let f = fixture("a: &a\n  name: A\nb: &b\n  name: B\n  size: big\nc:\n  <<: [*a, *b]\n")
        .expect("ingest");
    assert_eq!(field(&f, "c", "name").as_deref(), Some("A"));
    assert_eq!(field(&f, "c", "size").as_deref(), Some("big"));
}

/// `_fixture:` configures the set. Its `ignore` rows exist only to be
/// merged into others, so they are not inserted, but the rows merging
/// them keep their fields.
#[test]
fn fixture_config_is_not_a_row_and_ignored_rows_are_dropped() {
    let f = fixture(
        "_fixture:\n  model_class: Widget\n  ignore:\n    - DEFAULTS\n\
         DEFAULTS: &DEFAULTS\n  color: red\n\
         one:\n  <<: *DEFAULTS\n  name: One\n",
    )
    .expect("ingest");
    let labels: Vec<&str> = f.records.keys().map(|k| k.as_str()).collect();
    assert_eq!(labels, ["one"]);
    assert_eq!(field(&f, "one", "color").as_deref(), Some("red"));
    assert_eq!(f.model_class.as_ref().map(|c| c.as_str()), Some("Widget"));
}

/// A single `ignore: DEFAULTS` is a one-label list.
#[test]
fn a_scalar_ignore_names_one_label() {
    let f = fixture("_fixture:\n  ignore: DEFAULTS\nDEFAULTS: &d\n  a: 1\none:\n  <<: *d\n")
        .expect("ingest");
    let labels: Vec<&str> = f.records.keys().map(|k| k.as_str()).collect();
    assert_eq!(labels, ["one"]);
}

/// A YAML file under `test/fixtures` that isn't label → fields is data a
/// test reads, not a fixture set. It is reported as such rather than as
/// a parse error of the file.
#[test]
fn a_sequence_at_the_top_is_not_a_fixture_set() {
    let err = fixture("- one\n- two\n").expect_err("a sequence is not a set");
    match err {
        IngestError::Unsupported { message, .. } => {
            assert!(message.contains("not a fixture set"), "{message}")
        }
        other => panic!("expected Unsupported, got {other}"),
    }
}

fn app(files: &[(&str, &str)]) -> roundhouse::app::App {
    let tree = files
        .iter()
        .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
        .collect();
    roundhouse::ingest::ingest_app_from_tree(tree).expect("ingest tree")
}

const SCHEMA: &str = "ActiveRecord::Schema.define(version: 1) do\n  \
                      create_table :widgets do |t|\n    t.string :name\n  end\nend\n";

/// `model_class` names the class the rows load, where the path would
/// name another (`legacy/things` -> `Legacy::Thing`, which does not
/// exist); without it the model is not found and every field drops.
#[test]
fn model_class_picks_the_model_the_rows_load() {
    let app = app(&[
        ("db/schema.rb", SCHEMA),
        ("app/models/widget.rb", "class Widget < ApplicationRecord\nend\n"),
        ("test/fixtures/legacy/things.yml", "_fixture:\n  model_class: Widget\none:\n  name: One\n"),
    ]);
    let lowered = roundhouse::lower::lower_fixtures(&app);
    let set = &lowered.fixtures[0];
    assert_eq!(set.class.0.as_str(), "Widget");
    assert_eq!(set.records[0].fields.len(), 1);
}

/// `test/fixtures/files/` is `file_fixture_path`; Rails leaves it out
/// of the fixture sets even when the files there are YAML.
#[test]
fn yaml_under_file_fixtures_is_not_a_fixture_set() {
    let app = app(&[
        ("db/schema.rb", SCHEMA),
        ("app/models/widget.rb", "class Widget < ApplicationRecord\nend\n"),
        ("test/fixtures/widgets.yml", "one:\n  name: One\n"),
        ("test/fixtures/files/payload.yml", "- a\n- b\n"),
    ]);
    let names: Vec<&str> = app.fixtures.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["widgets"]);
}
