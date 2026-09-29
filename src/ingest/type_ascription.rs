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

/// Wrap `value` in a `Cast` to `ty`, unless the type is unreadable
/// (`None` or an open inference variable), in which case `value` is
/// returned as is. `untyped` IS a reading: `T.unsafe(x)` and
/// `x #: as untyped` are the author's signed escape hatch, so the value
/// leaves the type system rather than keeping the type inferred for it.
pub(super) fn ascribe(value: Expr, ty: Option<Ty>) -> Expr {
    match ty {
        Some(ty) if !ty.is_open() => {
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
    // `T.unsafe(x)` is the escape hatch: no second argument, the value
    // is untyped from there on.
    if method == "unsafe" {
        return Some(Ty::Untyped);
    }
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

/// The type of a `#: as Type` assertion written between a call's
/// receiver and the leading-dot line that continues the call:
///
/// ```text
/// self #: as untyped
///   .before_update(prepend: true) { ... }
/// ```
///
/// The comment ends the receiver's own line, so `trailing_rbs_type`
/// reads it as the receiver's declared type. Only the `as` form counts
/// here, and only when the next non-blank line continues with `.` or
/// `&.`; a comment after a receiver on the same line as its method
/// (`x.foo #: as T`) never gets this far, because the text after the
/// receiver is then the method, not the comment.
pub(super) fn receiver_rbs_assertion(file: &str, end: usize) -> Option<Ty> {
    let text = sources::text_of(file)?;
    let rest = text.get(end..)?;
    let mut lines = rest.split('\n');
    let comment = lines.next()?.trim_start().strip_prefix("#:")?.trim();
    comment.strip_prefix("as ")?;
    let continues = lines
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .is_some_and(|l| {
            let l = l.trim_start();
            l.starts_with('.') || l.starts_with("&.")
        });
    if !continues {
        return None;
    }
    trailing_rbs_type(file, end)
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

