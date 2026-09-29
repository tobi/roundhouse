//! `...` argument forwarding: `def m(...) = other(...)`.
//!
//! `def m(...)` accepts any positionals, keywords and block, and `other(...)`
//! passes all three on unchanged; it is `*rest, **kw, &blk` without the
//! names. Core uses it for delegators (`each(...)`, `open(...)`,
//! `T.unsafe(self).checkout_token(...)`). Ingest used to refuse the
//! `ForwardingArgumentsNode` and drop the whole file with it.
//!
//! We bind the rest and the block to fixed synthetic names, `__fwd` and
//! `__blk` (the binding an anonymous `&` already gets), and splat and pass
//! them at the call. Keywords ride in the rest as a trailing Hash, the
//! convention `*args, **opts` already follows.

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
        ("app/helpers/proxy.rb", body.to_string()),
    ];
    let tree: HashMap<PathBuf, Vec<u8>> = files
        .into_iter()
        .map(|(p, c)| (PathBuf::from(p), c.into_bytes()))
        .collect();
    let app = ingest_app_from_tree(tree).expect("`...` forwarding must ingest");
    ruby::emit_library(&app)
        .iter()
        .find(|f| f.path.to_string_lossy().ends_with("proxy.rb"))
        .map(|f| f.content.clone())
        .expect("proxy.rb")
}

#[test]
fn a_forwarding_def_passes_positionals_and_block_to_the_call() {
    let out = emit(
        r#"class Proxy
  def each(...)
    @set.each(...)
  end
end
"#,
    );
    assert!(out.contains("def each(*__fwd, &__blk)"), "got:\n{out}");
    assert!(out.contains("@set.each(*__fwd, &__blk)"), "got:\n{out}");
}

#[test]
fn leading_parameters_stay_ahead_of_the_forwarded_rest() {
    let out = emit(
        r#"class Proxy
  def open_with(mode, ...)
    open(mode, ...)
  end
end
"#,
    );
    assert!(out.contains("def open_with(mode, *__fwd, &__blk)"), "got:\n{out}");
    assert!(out.contains("open(mode, *__fwd, &__blk)"), "got:\n{out}");
}

#[test]
fn super_can_forward_too() {
    let out = emit(
        r#"class Base
  def go(*a); end
end

class Proxy < Base
  def go(...)
    super(...)
  end
end
"#,
    );
    assert!(out.contains("def go(*__fwd, &__blk)"), "got:\n{out}");
    assert!(out.contains("super(*__fwd"), "got:\n{out}");
}
