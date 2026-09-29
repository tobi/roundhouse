//! Which gem owns a constant: only a gem that is NOT in the tree, and
//! only if no other locked gem spells the constant out in full.
//!
//! The attribution pass downgrades a failed dispatch on a gem's
//! constants to a coverage note ("the `X` gem is in the Gemfile and
//! roundhouse does not model it"). Two claims in it were wrong:
//!
//! * a `PATH` gem is a component of the repository, its source is in
//!   the tree under analysis, so a dispatch failing on `Apps::Api` is a
//!   fact about that code, not a gem boundary;
//! * `benchmark-ips` was credited with `Benchmark::…` by its first
//!   name segment while `benchmark` (the gem that IS `Benchmark`) sat
//!   in the same lockfile.

use roundhouse::gems::{gem_owning_constant, GemCensus, GemFate, Lockfile};

const LOCK: &str = "\
PATH
  remote: components/apps
  specs:
    apps (0.1.0)

GEM
  remote: https://rubygems.org/
  specs:
    benchmark (0.5.0)
    benchmark-ips (2.14.0)
    redcarpet (3.6.1)

PLATFORMS
  ruby

DEPENDENCIES
  apps!
  benchmark
  benchmark-ips
  redcarpet
";

fn census() -> GemCensus {
    GemCensus::of(&Lockfile::parse(LOCK))
}

#[test]
fn a_path_gem_is_in_the_repo_not_unknown() {
    let lock = Lockfile::parse(LOCK);
    assert!(lock.is_in_repo("apps"));
    assert!(!lock.is_in_repo("redcarpet"));
    let census = census();
    let apps = census.gems.iter().find(|g| g.name == "apps").unwrap();
    assert_eq!(apps.fate, GemFate::InRepo);
    assert!(census.unknown().all(|g| g.name != "apps"));
}

#[test]
fn a_constant_of_an_in_repo_gem_is_not_attributed_to_a_missing_gem() {
    assert_eq!(gem_owning_constant(&census(), "Apps::Api"), None);
}

#[test]
fn a_real_external_gem_still_owns_its_constant() {
    assert_eq!(gem_owning_constant(&census(), "Redcarpet::Markdown"), Some("redcarpet"));
}

#[test]
fn a_first_segment_claim_yields_to_the_gem_that_spells_the_constant() {
    let census = census();
    assert_eq!(gem_owning_constant(&census, "Benchmark::Tms"), None);
    assert_eq!(gem_owning_constant(&census, "Benchmark::IPS::Job"), None);
}
