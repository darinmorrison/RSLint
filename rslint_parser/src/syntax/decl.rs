//! Class and function declarations.

use super::expr::{assign_expr, object_prop_name, lhs_expr, EXPR_RECOVERY_SET};
use super::pat::{binding_element, binding_identifier, opt_binding_identifier, pattern};
use super::stmt::block_stmt;
use crate::{SyntaxKind::*, *};
use std::collections::HashMap;

/// A function declaration, this could be async and or a generator. This takes a marker
/// because you need to first advance over async or start a marker and feed it in.
// test function_decl
// function foo() {}
// function *foo() {}
// function foo(await) {}
// async function *foo() {}
// async function foo() {}
// function *foo() {
//   yield foo;
// }
pub fn function_decl(p: &mut Parser, m: Marker, fn_expr: bool) -> CompletedMarker {
    // test_err function_decl_err
    // function() {}
    // function *() {}
    // async function() {}
    // async function *() {}
    // function *foo() {}
    // yield foo;
    p.expect(T![function]);
    let in_generator = p.eat(T![*]);

    let complete = opt_binding_identifier(p);
    if complete.is_none() && !fn_expr {
        let err = p.err_builder("expected a name for the function in a function declaration, but found none")
            .primary(p.cur_tok().range, "");

        p.error(err);
    }
    formal_parameters(p);

    block_stmt(
        &mut *p.with_state(ParserState {
            labels: HashMap::new(),
            in_function: true,
            in_generator,
            ..p.state.clone()
        }),
        true,
        None,
    );
    m.complete(p, FN_DECL)
}

pub fn formal_parameters(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    let mut first = true;

    p.expect(T!['(']);

    while !p.at(EOF) && !p.at(T![')']) {
        if first {
            first = false;
        } else if p.nth_at(1, T![')']) {
            p.eat(T![,]);
            break;
        } else {
            p.expect(T![,]);
        }

        if p.at(T![...]) {
            let m = p.start();
            p.bump_any();
            pattern(p);
            m.complete(p, REST_PATTERN);
            break;
        }

        binding_element(p);
    }

    p.expect(T![')']);
    m.complete(p, PARAMETER_LIST)
}

pub fn arrow_body(p: &mut Parser) -> Option<CompletedMarker> {
    let mut guard = p.with_state(ParserState {
        in_function: true,
        ..p.state.clone()
    });
    if guard.at(T!['{']) {
        Some(block_stmt(&mut *guard, true, None))
    } else {
        assign_expr(&mut *guard)
    }
}

// test class_decl
// class foo {}
// class foo extends bar {}
// class foo extends foo.bar {}
pub fn class_decl(p: &mut Parser, expr: bool) -> CompletedMarker {
    // test_err class_decl_err
    // class {}
    // class extends bar {}
    // class extends {}
    // class
    let m = p.start();
    p.expect(T![class]);
    // class bodies are implicitly strict
    let mut guard = p.with_state(ParserState {
        strict: Some(StrictMode::Class(p.cur_tok().range)),
        ..p.state.clone()
    });

    if !guard.at(T!['{']) && !guard.at(T![extends]) {
        binding_identifier(&mut *guard);
    } else if !expr {
        let err = guard
            .err_builder("class declarations must have a name")
            .primary(guard.cur_tok(), "");

        guard.error(err);
    }

    if guard.eat(T![extends]) {
        lhs_expr(&mut *guard);
    }

    class_body(&mut *guard);

    m.complete(&mut *guard, CLASS_DECL)
}

fn class_body(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(T!['{']);

    while !p.at(EOF) && !p.at(T!['}']) {
        match p.cur() {
            // test class_empty_element
            // class foo { ;;;;;;;;;; get foo() {};;;;}
            T![;] => {
                let inner = p.start();
                p.bump_any();
                inner.complete(p, EMPTY_STMT);
            }
            // test static_method
            // class foo {
            //  static foo(bar) {}
            //  static *foo() {}
            //  static async foo() {}
            //  static async *foo() {}
            // }
            _ if p.cur_src() == "static" => {
                let inner = p.start();
                p.bump_any();
                method(p, None);
                inner.complete(p, STATIC_METHOD);
            }
            _ => {
                method(p, None);
            }
        }
    }
    p.expect(T!['}']);
    m.complete(p, CLASS_BODY)
}

/// A method definition, this takes an optional markers for object props
pub fn method(p: &mut Parser, marker: impl Into<Option<Marker>>) -> Option<CompletedMarker> {
    let m = marker.into().unwrap_or_else(|| p.start());
    let old = p.state.to_owned();
    p.state.in_function = true;
    // FIXME: handle get* which is a property + a generator
    let complete = match p.cur() {
        // FIXME: this is wrong and it wrongfully allows things like `class foo { (bar) {} }`
        T!['('] => {
            formal_parameters(p);
            block_stmt(p, true, None);
            m.complete(p, METHOD)
        }
        // test method_getter
        // class foo {
        //  get bar() {}
        // }

        // test_err method_getter_err
        // class foo {
        //  get {}
        // }
        T![ident] if p.cur_src() == "get" && p.nth(1) != T!['('] => {
            p.bump_any();
            object_prop_name(p, false);
            p.expect(T!['(']);
            p.expect(T![')']);
            block_stmt(p, true, None);
            m.complete(p, GETTER)
        }
        // test method_setter
        // class foo {
        //  set bar() {}
        // }
        T![ident] if p.cur_src() == "set" && p.nth(1) != T!['('] => {
            p.bump_any();
            object_prop_name(p, false);
            formal_parameters(p);
            block_stmt(p, true, None);
            m.complete(p, SETTER)
        }
        // test async_method
        // class foo {
        //  async foo() {}
        //  async *foo() {}
        // }
        T![ident] if p.cur_src() == "async" && !p.has_linebreak_before_n(1) => {
            p.bump_any();
            let in_generator = p.eat(T![*]);
            let mut guard = p.with_state(ParserState {
                in_async: true,
                in_generator,
                ..p.state.clone()
            });
            object_prop_name(&mut *guard, true);
            formal_parameters(&mut *guard);
            block_stmt(&mut *guard, true, None);
            drop(guard);
            m.complete(p, METHOD)
        }
        T![*] | STRING | NUMBER | T![await] | T![ident] | T![yield] | T!['['] => {
            let in_generator = p.eat(T![*]);
            let mut guard = p.with_state(ParserState {
                in_generator,
                ..p.state.clone()
            });
            object_prop_name(&mut *guard, true);
            formal_parameters(&mut *guard);
            block_stmt(&mut *guard, true, None);
            drop(guard);
            m.complete(p, METHOD)
        }
        t if t.is_keyword() => {
            let in_generator = p.eat(T![*]);
            let mut guard = p.with_state(ParserState {
                in_generator,
                ..p.state.clone()
            });
            object_prop_name(&mut *guard, false);
            formal_parameters(&mut *guard);
            block_stmt(&mut *guard, true, None);
            drop(guard);
            m.complete(p, METHOD)
        }
        _ => {
            let err = p
                .err_builder("Expected a method definition, but found none")
                .primary(p.cur_tok(), "");

            p.err_recover(err, EXPR_RECOVERY_SET);
            return None;
        }
    };
    p.state = old;
    Some(complete)
}
