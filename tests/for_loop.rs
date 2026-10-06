//! A for loop binds in its enclosing scope; an each block cannot represent it.
fn refused(body: &str) {
    let result = ruby_prism::parse(body.as_bytes());
    let program = result.node().as_program_node().unwrap();
    let node = program.statements().body().iter().find(|node| node.as_for_node().is_some()).expect("for input");
    let error = roundhouse::ingest::ingest_expr(&node, body).expect_err("for must not become each").to_string();
    assert!(error.contains("for") && error.contains("scope"), "{error}");
}

#[test]
fn enclosing_scope_binding_is_refused() { refused("x = 10; for x in items; end; x"); }

#[test]
fn destructuring_is_refused() { refused("for key, value in items; puts key, value; end"); }

#[test]
fn loop_control_does_not_hide_scope_gap() { refused("for n in items; next if n.odd?; break; end"); }
