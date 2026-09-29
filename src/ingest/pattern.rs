//! `case … in` pattern matching, desugared to the `if` ladder it means.
//!
//! The IR's `Case`/`Pattern` pair models `case … when` (`===` dispatch,
//! plus a few structural forms the fixtures use), not Ruby's `in`
//! patterns with their nesting, alternation, pins and captures. Rather
//! than widen every emitter and lowerer, a `case/in` compiles here into
//! ordinary expressions:
//!
//! ```ruby
//! case [a, b]
//! in [:x, Foo | nil] then 1
//! else 2
//! end
//! ```
//!
//! becomes
//!
//! ```ruby
//! __in_N = [a, b]
//! if __in_N.is_a?(Array) && __in_N.size == 2 && :x === __in_N[0] &&
//!    (Foo === __in_N[1] || __in_N[1].nil?)
//!   1
//! else
//!   2
//! end
//! ```
//!
//! Each pattern yields TESTS (pure boolean expressions over projections
//! of the scrutinee) and BINDINGS (locals assigned from those same
//! projections). The tests decide the arm; the bindings are assigned at
//! the top of the arm's body, before the guard if there is one. With no
//! `else`, falling off the ladder raises `NoMatchingPatternError`, as
//! Ruby does.
//!
//! Array and hash patterns without a constant match Arrays and Hashes
//! directly (`is_a?`, `[]`, `key?`); with a constant (`Point[x, y]`,
//! `Point(x:)`) the constant is checked with `===` and the value is
//! read through `deconstruct` / `deconstruct_keys`. Find patterns
//! (`[*, x, *]`) are not supported.

use ruby_prism::Node;

use super::expr::ingest_expr;
use super::util::constant_id_str;
use crate::expr::{BoolOpKind, BoolOpSurface, Expr, ExprNode, LValue, Literal};
use crate::ident::{Symbol, VarId};
use crate::span::Span;
use super::{IngestError, IngestResult};

fn mk(node: ExprNode) -> Expr {
    Expr::new(Span::synthetic(), node)
}

fn send(recv: Expr, method: &str, args: Vec<Expr>) -> Expr {
    mk(ExprNode::Send {
        recv: Some(recv),
        method: Symbol::from(method),
        args,
        block: None,
        parenthesized: true,
    })
}

fn int(v: i64) -> Expr {
    mk(ExprNode::Lit { value: Literal::Int { value: v } })
}

fn sym(s: &str) -> Expr {
    mk(ExprNode::Lit { value: Literal::Sym { value: Symbol::from(s) } })
}

fn var(name: &Symbol) -> Expr {
    mk(ExprNode::Var { id: VarId(0), name: name.clone() })
}

fn bool_op(op: BoolOpKind, left: Expr, right: Expr) -> Expr {
    mk(ExprNode::BoolOp { op, surface: BoolOpSurface::Symbol, left, right })
}

fn and_all(tests: Vec<Expr>) -> Expr {
    let mut it = tests.into_iter();
    match it.next() {
        None => mk(ExprNode::Lit { value: Literal::Bool { value: true } }),
        Some(first) => it.fold(first, |l, r| bool_op(BoolOpKind::And, l, r)),
    }
}

/// What one pattern demands of the value it is matched against.
#[derive(Default)]
struct Compiled {
    tests: Vec<Expr>,
    binds: Vec<(Symbol, Expr)>,
}

fn unsupported(file: &str, what: String) -> IngestError {
    IngestError::Unsupported { file: file.into(), message: what }
}

/// Ingest `case scrutinee; in … end`.
pub(super) fn ingest_case_match(
    c: &ruby_prism::CaseMatchNode<'_>,
    span: Span,
    file: &str,
) -> IngestResult<Expr> {
    let scrutinee = ingest_expr(&c.predicate().ok_or_else(|| {
        unsupported(file, "`case/in` without a subject".to_string())
    })?, file)?;
    let tmp = Symbol::from(format!("__in_{}", span.start).as_str());

    // The ladder's last rung. No `else` means no match is an error.
    let mut chain = match c.else_clause() {
        Some(e) => match e.statements() {
            Some(s) => ingest_expr(&s.as_node(), file)?,
            None => mk(ExprNode::Lit { value: Literal::Nil }),
        },
        None => mk(ExprNode::Raise {
            value: send(
                mk(ExprNode::Const { path: vec![Symbol::from("NoMatchingPatternError")] }),
                "new",
                vec![send(var(&tmp), "inspect", vec![])],
            ),
        }),
    };

    let clauses: Vec<_> = c.conditions().iter().collect();
    for clause in clauses.iter().rev() {
        let arm = clause.as_in_node().ok_or_else(|| {
            unsupported(file, format!("unsupported case/in clause: {clause:?}"))
        })?;
        let whole = arm.pattern();
        let peeled = peel_guard(&whole, file)?;
        let (pattern, guard) = match &peeled {
            Some((inner, guard)) => (inner, Some(guard.clone())),
            None => (&whole, None),
        };
        let mut compiled = Compiled::default();
        compile(pattern, &var(&tmp), &mut compiled, file)?;

        let body = match arm.statements() {
            Some(s) => ingest_expr(&s.as_node(), file)?,
            None => mk(ExprNode::Lit { value: Literal::Nil }),
        };
        let assign_binds = |binds: Vec<(Symbol, Expr)>, tail: Expr| -> Expr {
            if binds.is_empty() {
                return tail;
            }
            let mut exprs: Vec<Expr> = binds
                .into_iter()
                .map(|(name, value)| {
                    mk(ExprNode::Assign {
                        target: LValue::Var { id: VarId(0), name },
                        value,
                    })
                })
                .collect();
            exprs.push(tail);
            mk(ExprNode::Seq { exprs })
        };

        let Compiled { tests, binds } = compiled;
        let cond = and_all(tests);
        chain = match guard {
            None => mk(ExprNode::If {
                cond,
                then_branch: assign_binds(binds, body),
                else_branch: chain,
            }),
            // A guard reads the pattern's bindings, so they are assigned
            // first; a failed guard falls to the next rung, which is
            // therefore reachable from two places.
            Some(guard) if !binds.is_empty() => mk(ExprNode::If {
                cond,
                then_branch: assign_binds(
                    binds,
                    mk(ExprNode::If {
                        cond: guard,
                        then_branch: body,
                        else_branch: chain.clone(),
                    }),
                ),
                else_branch: chain,
            }),
            Some(guard) => mk(ExprNode::If {
                cond: bool_op(BoolOpKind::And, cond, guard),
                then_branch: body,
                else_branch: chain,
            }),
        };
    }

    Ok(mk(ExprNode::Seq {
        exprs: vec![
            mk(ExprNode::Assign {
                target: LValue::Var { id: VarId(0), name: tmp },
                value: scrutinee,
            }),
            chain,
        ],
    }))
}

/// `in pattern if guard` parses as the pattern wrapped in an `IfNode`
/// (`unless` likewise); peel it off, returning the inner pattern and the
/// guard as an expression.
fn peel_guard<'a>(node: &Node<'a>, file: &str) -> IngestResult<Option<(Node<'a>, Expr)>> {
    let (predicate, statements, negate) = if let Some(i) = node.as_if_node() {
        (i.predicate(), i.statements(), false)
    } else if let Some(u) = node.as_unless_node() {
        (u.predicate(), u.statements(), true)
    } else {
        return Ok(None);
    };
    let inner = statements
        .and_then(|s| s.body().iter().next())
        .ok_or_else(|| unsupported(file, "empty pattern guard".to_string()))?;
    let mut guard = ingest_expr(&predicate, file)?;
    if negate {
        guard = send(guard, "!", vec![]);
    }
    Ok(Some((inner, guard)))
}

fn bind(out: &mut Compiled, name: &str, value: &Expr) {
    // `_` and `_x` are the conventional "don't care" names.
    if name != "_" {
        out.binds.push((Symbol::from(name), value.clone()));
    }
}

/// Compile `node` matched against the value `v`.
fn compile(node: &Node<'_>, v: &Expr, out: &mut Compiled, file: &str) -> IngestResult<()> {
    // `x` — bind whatever is there.
    if let Some(t) = node.as_local_variable_target_node() {
        bind(out, constant_id_str(&t.name()), v);
        return Ok(());
    }
    // `P => x` — must match P, and binds the whole value.
    if let Some(cap) = node.as_capture_pattern_node() {
        compile(&cap.value(), v, out, file)?;
        let target = cap.target();
        bind(out, constant_id_str(&target.name()), v);
        return Ok(());
    }
    // `A | B` — either side; neither may bind (Ruby forbids it too).
    if let Some(alt) = node.as_alternation_pattern_node() {
        let mut sides = Vec::new();
        for side in [alt.left(), alt.right()] {
            let mut c = Compiled::default();
            compile(&side, v, &mut c, file)?;
            if !c.binds.is_empty() {
                return Err(unsupported(file, "a variable bound inside an alternation".into()));
            }
            sides.push(and_all(c.tests));
        }
        let right = sides.pop().unwrap();
        let left = sides.pop().unwrap();
        out.tests.push(bool_op(BoolOpKind::Or, left, right));
        return Ok(());
    }
    // `(P)`
    if let Some(p) = node.as_parentheses_node() {
        if let Some(body) = p.body().and_then(|b| b.as_statements_node()) {
            let mut inner = body.body().iter();
            if let (Some(only), None) = (inner.next(), inner.next()) {
                return compile(&only, v, out, file);
            }
        }
    }
    // `^x`, `^(expr)`, `^@x` — compare with `===` against the current value.
    if let Some(pin) = node.as_pinned_variable_node() {
        let e = ingest_expr(&pin.variable(), file)?;
        out.tests.push(send(e, "===", vec![v.clone()]));
        return Ok(());
    }
    if let Some(pin) = node.as_pinned_expression_node() {
        let e = ingest_expr(&pin.expression(), file)?;
        out.tests.push(send(e, "===", vec![v.clone()]));
        return Ok(());
    }
    if let Some(a) = node.as_array_pattern_node() {
        return compile_array(&a, v, out, file);
    }
    if let Some(h) = node.as_hash_pattern_node() {
        return compile_hash(&h, v, out, file);
    }
    if node.as_find_pattern_node().is_some() {
        return Err(unsupported(file, "find pattern (`[*, x, *]`)".into()));
    }
    // `nil` reads better as a predicate than as `nil === v`.
    if node.as_nil_node().is_some() {
        out.tests.push(send(v.clone(), "nil?", vec![]));
        return Ok(());
    }
    // Everything else is a value pattern: a constant, a literal, a range,
    // a regexp, a lambda — matched by `pattern === value`.
    let e = ingest_expr(node, file)?;
    out.tests.push(send(e, "===", vec![v.clone()]));
    Ok(())
}

/// The value's positional elements: the value itself when it is a bare
/// array pattern (`[a, b]`), `value.deconstruct` behind a constant.
fn compile_array(
    a: &ruby_prism::ArrayPatternNode<'_>,
    v: &Expr,
    out: &mut Compiled,
    file: &str,
) -> IngestResult<()> {
    let elems = match a.constant() {
        Some(c) => {
            let k = ingest_expr(&c, file)?;
            out.tests.push(send(k, "===", vec![v.clone()]));
            send(v.clone(), "deconstruct", vec![])
        }
        None => {
            out.tests.push(send(
                v.clone(),
                "is_a?",
                vec![mk(ExprNode::Const { path: vec![Symbol::from("Array")] })],
            ));
            v.clone()
        }
    };
    let requireds: Vec<Node<'_>> = a.requireds().iter().collect();
    let posts: Vec<Node<'_>> = a.posts().iter().collect();
    let fixed = (requireds.len() + posts.len()) as i64;
    let size = send(elems.clone(), "size", vec![]);
    out.tests.push(if a.rest().is_some() {
        send(size, ">=", vec![int(fixed)])
    } else {
        send(size, "==", vec![int(fixed)])
    });

    for (i, p) in requireds.iter().enumerate() {
        compile(p, &send(elems.clone(), "[]", vec![int(i as i64)]), out, file)?;
    }
    if let Some(rest) = a.rest() {
        // `*name` binds the slice between the leading and trailing
        // elements; `*` alone (or an implicit trailing comma) binds nothing.
        if let Some(splat) = rest.as_splat_node() {
            if let Some(target) = splat.expression().and_then(|e| e.as_local_variable_target_node()) {
                let start = requireds.len() as i64;
                let slice = mk(ExprNode::Range {
                    begin: Some(int(start)),
                    end: if posts.is_empty() { None } else { Some(int(-(posts.len() as i64) - 1)) },
                    exclusive: false,
                });
                bind(out, constant_id_str(&target.name()), &send(elems.clone(), "[]", vec![slice]));
            }
        }
    }
    let m = posts.len() as i64;
    for (j, p) in posts.iter().enumerate() {
        let idx = -(m - j as i64);
        compile(p, &send(elems.clone(), "[]", vec![int(idx)]), out, file)?;
    }
    Ok(())
}

/// `{ k: pattern, … }` and `Const(k: pattern)`. Keys must be present;
/// `**rest` binds the remaining pairs and `**nil` demands there be none.
fn compile_hash(
    h: &ruby_prism::HashPatternNode<'_>,
    v: &Expr,
    out: &mut Compiled,
    file: &str,
) -> IngestResult<()> {
    let pairs = match h.constant() {
        Some(c) => {
            let k = ingest_expr(&c, file)?;
            out.tests.push(send(k, "===", vec![v.clone()]));
            send(v.clone(), "deconstruct_keys", vec![mk(ExprNode::Lit { value: Literal::Nil })])
        }
        None => {
            out.tests.push(send(
                v.clone(),
                "is_a?",
                vec![mk(ExprNode::Const { path: vec![Symbol::from("Hash")] })],
            ));
            v.clone()
        }
    };
    let mut keys: Vec<Expr> = Vec::new();
    let mut n_keys = 0i64;
    for el in h.elements().iter() {
        let Some(assoc) = el.as_assoc_node() else {
            return Err(unsupported(file, format!("unsupported hash pattern element: {el:?}")));
        };
        let key_node = assoc.key();
        let key_name = super::util::symbol_value(&key_node)
            .ok_or_else(|| unsupported(file, format!("non-symbol hash pattern key: {key_node:?}")))?;
        let key = sym(&key_name);
        keys.push(key.clone());
        n_keys += 1;
        out.tests.push(send(pairs.clone(), "key?", vec![key.clone()]));
        let value = send(pairs.clone(), "[]", vec![key]);
        let pv = assoc.value();
        if let Some(implicit) = pv.as_implicit_node() {
            // `{ name: }` — shorthand binding a local of the key's name.
            let _ = implicit;
            bind(out, &key_name, &value);
        } else {
            compile(&pv, &value, out, file)?;
        }
    }
    if let Some(rest) = h.rest() {
        if rest.as_no_keywords_parameter_node().is_some() {
            out.tests.push(send(send(pairs.clone(), "size", vec![]), "==", vec![int(n_keys)]));
        } else if let Some(splat) = rest.as_assoc_splat_node() {
            if let Some(target) = splat.value().and_then(|e| e.as_local_variable_target_node()) {
                bind(out, constant_id_str(&target.name()), &send(pairs.clone(), "except", keys));
            }
        }
    }
    Ok(())
}
