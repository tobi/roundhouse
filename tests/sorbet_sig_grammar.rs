//! The type forms a Sorbet `sig` may use beyond the everyday ones.
//!
//! A `sig` containing a form the reader did not know was dropped whole,
//! and the method was typed as nothing: `T.class_of` (1,100 signatures
//! in core), generic methods (`type_parameters` / `T.type_parameter`,
//! 280), `T.anything` (115), `T.all`, `T.noreturn`, shapes, tuples.

use roundhouse::ident::{ClassId, Symbol};
use roundhouse::ingest::sorbet_sig::ingest_sorbet_signatures;
use roundhouse::ty::{ParamKind, Ty};

/// The signature of `m` in the one class of `source`.
fn read(sig: &str, def: &str) -> Ty {
    let source = format!("class C\n  {sig}\n  {def}\nend\n");
    let sigs = ingest_sorbet_signatures(source.as_bytes());
    sigs.get(&ClassId(Symbol::new("C")))
        .and_then(|m| m.get(&Symbol::new("m")))
        .unwrap_or_else(|| panic!("`{sig}` should read"))
        .clone()
}

fn class(id: &str) -> Ty {
    Ty::Class { id: ClassId(Symbol::new(id)), args: vec![] }
}

fn signature(ty: &Ty) -> (Vec<Ty>, Ty) {
    let Ty::Fn { params, ret, .. } = ty else { panic!("{ty:?}") };
    (params.iter().map(|p| p.ty.clone()).collect(), (**ret).clone())
}

#[test]
fn class_of_is_the_class_it_names_even_for_a_builtin() {
    let ty = read(
        "sig { params(a: T.class_of(Cart), b: T.nilable(T.class_of(String))).returns(T.class_of(Cart)) }",
        "def m(a, b); end",
    );
    let (params, ret) = signature(&ty);
    assert_eq!(params[0], class("Cart"));
    assert_eq!(params[1], Ty::Union { variants: vec![class("String"), Ty::Nil] });
    assert_eq!(ret, class("Cart"));
}

#[test]
fn a_generic_method_reads_with_its_type_parameter_untyped() {
    let ty = read(
        "sig { type_parameters(:U).params(value: T.type_parameter(:U)).returns(T.type_parameter(:U)) }",
        "def m(value); end",
    );
    let (params, ret) = signature(&ty);
    assert_eq!(params[0], Ty::Untyped);
    assert_eq!(ret, Ty::Untyped);
}

#[test]
fn noreturn_anything_and_all() {
    let ty = read(
        "sig { params(a: T.anything, b: T.all(Cart, Kernel), c: T.all(Cart, Priced)).returns(T.noreturn) }",
        "def m(a, b, c); end",
    );
    let (params, ret) = signature(&ty);
    assert_eq!(params[0], Ty::Untyped);
    assert_eq!(params[1], class("Cart"), "Kernel adds nothing");
    assert_eq!(params[2], Ty::Untyped, "two real surfaces: claim neither");
    assert_eq!(ret, Ty::Bottom);
}

#[test]
fn shapes_tuples_and_parentheses() {
    let ty = read(
        "sig { params(a: [Integer, String], b: (Integer)).returns({ max: Integer, name: String }) }",
        "def m(a, b); end",
    );
    let (params, ret) = signature(&ty);
    assert_eq!(params[0], Ty::Tuple { elems: vec![Ty::Int, Ty::Str] });
    assert_eq!(params[1], Ty::Int);
    let Ty::Record { row } = ret else { panic!() };
    assert_eq!(row.fields[&Symbol::new("max")], Ty::Int);
    assert_eq!(row.fields[&Symbol::new("name")], Ty::Str);
}

#[test]
fn bind_on_a_sig_is_not_a_type() {
    let ty = read("sig { bind(Cart).params(a: Integer).void }", "def m(a); end");
    let Ty::Fn { params, .. } = &ty else { panic!() };
    assert_eq!(params[0].kind, ParamKind::Required);
}
