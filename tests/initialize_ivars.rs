//! An ivar `initialize` assigns is set before any other method runs.
//!
//! ```ruby
//! class Quote
//!   include ActiveModel::Model
//!
//!   def initialize
//!     @rate = 1.5
//!   end
//!
//!   def over?
//!     @rate > 1
//!   end
//! end
//! ```
//!
//! An ivar written by some method of the class is `T | nil` to every method
//! that could run before that write, so a memoized `@x ||= ...` is nilable
//! and reads of it are checked for nil. But `initialize` is the one method
//! that always runs first: an ivar it assigns as a statement of its own
//! (not in a branch or a block, where the write may never happen) is never
//! nil in the other methods, and `@rate > 1` was reported as `Float? > Integer`.
//! A `nil` some other method writes is still part of the type.

use roundhouse::analyze::{diagnose, Analyzer, DiagnosticKind};

fn binops(model: &str) -> Vec<String> {
    let tree: std::collections::HashMap<std::path::PathBuf, Vec<u8>> = [
        (
            "app/models/application_record.rb",
            "class ApplicationRecord < ActiveRecord::Base\nend\n",
        ),
        ("app/models/quote.rb", model),
        ("db/schema.rb", "ActiveRecord::Schema.define(version: 1) do\nend\n"),
    ]
    .iter()
    .map(|(p, c)| (std::path::PathBuf::from(p), c.as_bytes().to_vec()))
    .collect();
    let mut app = roundhouse::ingest::ingest_app_from_tree(tree).expect("ingest tree");
    Analyzer::new(&app).analyze(&mut app);
    diagnose(&app)
        .into_iter()
        .filter(|d| matches!(d.kind, DiagnosticKind::IncompatibleBinop { .. }))
        .map(|d| format!("{:?}", d.kind))
        .collect()
}

#[test]
fn an_ivar_initialize_assigns_is_not_nilable_elsewhere() {
    let found = binops(
        r#"class Quote
  include ActiveModel::Model

  def initialize
    @rate = 1.5
    @count = 3
  end

  def over?
    @rate > 1 && @count + 1 > 2
  end
end
"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn an_assignment_in_a_branch_or_another_methods_nil_still_counts() {
    let found = binops(
        r#"class Quote
  include ActiveModel::Model

  def initialize(flag)
    @rate = 1.5 if flag
    @count = 3
  end

  def clear
    @count = nil
  end

  def over?
    @rate > 1 && @count + 1 > 2
  end
end
"#,
    );
    assert_eq!(found.len(), 2, "{found:?}");
}
