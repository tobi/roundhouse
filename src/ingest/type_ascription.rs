//! Type ascriptions written in the source: `T.let(x, Type)` /
//! `T.cast(x, Type)` and the RBS inline forms `x = value #: Type` /
//! `x = value #: as Type`.
//!
//! All of them say, in the author's own words, what the value at that
//! spot is. Ingest used to throw the words away and keep the value
//! (sorbet-runtime's assertions evaluate to their first argument), so a
//! value the analyzer could not type stayed untyped even though the
//! source declared it — every read of `@preview_token` after
//! `@preview_token = T.let(result.ok_value, T.nilable(Token))` reported
//! `ivar_unresolved`. The declared type now rides along as an
//! [`ExprNode::Cast`], which the typer already reads as "this
//! expression has this type".

use ruby_prism::Node;

use crate::expr::{Expr, ExprNode};
use crate::ty::Ty;

use super::sorbet_sig::sorbet_type_node;
use super::sources;

/// Wrap `value` in a `Cast` to `ty`, unless the type says nothing
/// (`untyped` / unreadable), in which case `value` is returned as is.
pub(super) fn ascribe(value: Expr, ty: Option<Ty>) -> Expr {
    match ty {
        Some(ty) if !ty.is_open() && !matches!(ty, Ty::Untyped) => {
            let span = value.span;
            Expr::new(span, ExprNode::Cast { value, target_ty: ty })
        }
        _ => value,
    }
}

/// The declared type of a `T.let(x, Type)` / `T.cast(x, Type)` call.
pub(super) fn sorbet_declared_type(node: &Node<'_>) -> Option<Ty> {
    let call = node.as_call_node()?;
    let method = call.name();
    let method = std::str::from_utf8(method.as_slice()).ok()?;
    if !matches!(method, "let" | "cast") {
        return None;
    }
    let args = call.arguments()?.arguments();
    let ty = args.iter().nth(1)?;
    sorbet_type_node(&ty)
}

/// The RBS type in a `#: Type` (or `#: as Type`) comment that follows
/// the statement ending at byte `end` on the same line.
pub(super) fn trailing_rbs_type(file: &str, end: usize) -> Option<Ty> {
    let text = sources::text_of(file)?;
    let rest = text.get(end..)?;
    let line = rest.split('\n').next()?;
    let comment = line.trim_start().strip_prefix("#:")?.trim();
    let comment = comment.strip_prefix("as ").unwrap_or(comment).trim();
    rbs_type(comment)
}

/// Parse one RBS type expression by wrapping it in a method signature
/// the RBS reader already understands.
pub(crate) fn rbs_type(text: &str) -> Option<Ty> {
    if text.is_empty() || text.contains('\n') || text.starts_with('!') {
        return None;
    }
    let src = format!("class X\n  def m: () -> {text}\nend\n");
    let sigs = crate::rbs::parse_signatures(&src).ok()?;
    let (_, sig) = sigs.methods.into_iter().next()?;
    match sig {
        Ty::Fn { ret, .. } => Some(*ret),
        _ => None,
    }
}

