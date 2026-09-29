//! A sorbet `sig` returning a generic class (`Shopify::Adt::Result[Token, Error]`)
//! used to fall outside the reader's grammar, dropping the whole signature.
//! Core's token classes declare `from_token` this way inside
//! `class << self`, so callers of `Token.from_token(...)` lost the
//! declared `Result` and everything read off it.

use std::collections::HashMap;
use std::path::PathBuf;

use roundhouse::ingest::ingest_app_from_tree;
use roundhouse::ty::Ty;
use roundhouse::{ClassId, Symbol};

#[test]
fn a_generic_return_type_keeps_the_signature() {
    let src = r#"module Cust
  class Tok
    class << self
      sig do
        params(token: String, client_id: T.nilable(String)).returns(Cust::Res[Tok, String])
      end
      def from_token(token, client_id: nil)
        decode(token)
      end
    end
  end
end
"#;
    let tree: HashMap<PathBuf, Vec<u8>> = [
        ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
        ("app/models/cust.rb", src),
    ]
    .iter()
    .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
    .collect();
    let app = ingest_app_from_tree(tree).expect("ingest");
    let sig = app
        .rbs_signatures
        .get(&ClassId(Symbol::new("Cust::Tok")))
        .and_then(|m| m.get(&Symbol::new("from_token")))
        .expect("from_token keeps its signature");
    let Ty::Fn { ret, .. } = sig else { panic!("{sig:?}") };
    match &**ret {
        Ty::Class { id, args } => {
            assert_eq!(id.0.as_str(), "Cust::Res");
            assert_eq!(args.len(), 2);
        }
        other => panic!("{other:?}"),
    }
}
