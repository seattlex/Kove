//! Resolver tests: what each name refers to, and the diagnostics about
//! names. Types are `typecheck.rs`.
//!
//! The `assert_code` tests run the whole driver, so they also confirm the
//! resolver's diagnostics reach the user; the tests at the bottom go
//! through the resolver API directly to check the resolution map itself.

use kove_ast::{ExprKind, Item, Stmt};
use kove_resolver::Resolution;
use kove_tests::{codes, resolve};

#[track_caller]
fn assert_code(src: &str, code: &str) {
    let found = codes(src);
    assert!(
        found.contains(&code),
        "expected {code} for {src:?}, got {found:?}"
    );
}

// --- Diagnostics the resolver owns -----------------------------------------

#[test]
fn e0200_unknown_type() {
    assert_code("fn main() { let x: Strng = \"s\"; }", "E0200");
    assert_code("fn f(a: Nope) { }\nfn main() { }", "E0200");
    assert_code("fn main() { let x = Missing::Variant; }", "E0200");
    assert_code("fn main() { let x = Missing { a: 1 }; }", "E0200");
}

#[test]
fn e0201_unknown_variable() {
    assert_code("fn main() { println(missing); }", "E0201");
    assert_code("fn main() { missing = 1; }", "E0201");
    // Variables do not leak out of their block.
    assert_code("fn main() { { let inner = 1; } println(inner); }", "E0201");
    // The for-loop variable does not outlive the loop.
    assert_code("fn main() { for i in 0..3 { } println(i); }", "E0201");
}

#[test]
fn e0202_unknown_function() {
    assert_code("fn main() { missing(); }", "E0202");
}

#[test]
fn e0204_assignment_to_immutable() {
    assert_code("fn main() { let a = 1; a = 2; }", "E0204");
    assert_code(
        "struct P { x: Int }\nfn main() { let p = P { x: 1 }; p.x = 2; }",
        "E0204",
    );
    // Function parameters are immutable.
    assert_code("fn f(a: Int) { a = 1; }\nfn main() { }", "E0204");
    // The for-loop variable is immutable.
    assert_code("fn main() { for i in 0..3 { i = 5; } }", "E0204");
}

#[test]
fn e0205_duplicate_definitions() {
    assert_code("fn f() { }\nfn f() { }\nfn main() { }", "E0205");
    assert_code("struct S { }\nstruct S { }\nfn main() { }", "E0205");
    assert_code("struct S { }\nenum S { A }\nfn main() { }", "E0205");
    assert_code("struct S { a: Int, a: Int }\nfn main() { }", "E0205");
    assert_code("fn f(a: Int, a: Int) { }\nfn main() { }", "E0205");
    assert_code("fn println(x: Int) { }\nfn main() { }", "E0205");
}

#[test]
fn e0213_invalid_assignment_target() {
    assert_code(
        "fn f() -> Int { return 1; }\nfn main() { f() = 2; }",
        "E0213",
    );
}

#[test]
fn e0216_unknown_variant() {
    assert_code("enum S { A }\nfn main() { let s = S::B; }", "E0216");
    assert_code("struct P { x: Int }\nfn main() { let s = P::A; }", "E0216");
}

#[test]
fn e0217_imports_not_supported_yet() {
    assert_code("import std::io;\nfn main() { }", "E0217");
}

#[test]
fn e0219_struct_literal_of_enum() {
    assert_code("enum S { A }\nfn main() { let s = S { a: 1 }; }", "E0219");
}

#[test]
fn e0230_callee_must_be_a_name() {
    assert_code(
        "struct P { x: Int }\nfn main() { let p = P { x: 1 }; p.x(); }",
        "E0230",
    );
}

// --- The resolution map ----------------------------------------------------

/// The `Var` expressions in the first function's body, in source order.
fn var_refs(program: &kove_ast::Program) -> Vec<(String, kove_ast::NodeId)> {
    let Item::Function(f) = &program.items[0] else {
        panic!("expected a function");
    };
    let mut out = Vec::new();
    for stmt in &f.body.stmts {
        if let Stmt::Let { value, .. } = stmt {
            if let ExprKind::Var(name) = &value.kind {
                out.push((name.clone(), value.id));
            }
        }
    }
    out
}

#[test]
fn a_reference_resolves_to_the_binding_that_is_in_scope() {
    // Both `b` and `c` read a variable called `a`, but they are different
    // bindings: the inner `let a` shadows the outer one.
    let (program, res, diags) = resolve(
        "fn main() {\n\
             let a = 1;\n\
             let b = a;\n\
         }",
    );
    assert!(diags.is_empty(), "{diags:?}");
    let refs = var_refs(&program);
    assert_eq!(refs.len(), 1);
    let Resolution::Local(local) = res.resolution(refs[0].1) else {
        panic!("`a` should resolve to a local");
    };
    assert_eq!(res.local(local).name, "a");
    assert!(!res.local(local).mutable);
}

#[test]
fn shadowing_creates_a_second_distinct_binding() {
    let (program, res, diags) = resolve(
        "fn main() {\n\
             let a = 1;\n\
             let outer = a;\n\
             let a = 2;\n\
             let inner = a;\n\
         }",
    );
    assert!(diags.is_empty(), "{diags:?}");
    let refs = var_refs(&program);
    assert_eq!(refs.len(), 2);
    let (Resolution::Local(first), Resolution::Local(second)) =
        (res.resolution(refs[0].1), res.resolution(refs[1].1))
    else {
        panic!("both references should resolve to locals");
    };
    assert_ne!(first, second, "shadowing must produce distinct bindings");
}

#[test]
fn mutability_is_recorded_on_the_binding() {
    let (program, res, _) = resolve("fn main() { let mut a = 1; let b = a; }");
    let refs = var_refs(&program);
    let Resolution::Local(local) = res.resolution(refs[0].1) else {
        panic!("expected a local");
    };
    assert!(res.local(local).mutable);
}

#[test]
fn items_resolve_regardless_of_declaration_order() {
    let (_, res, diags) = resolve(
        "fn main() { let p = make(); }\n\
         fn make() -> P { return P { x: 1 }; }\n\
         struct P { x: Int }",
    );
    assert!(diags.is_empty(), "{diags:?}");
    let id = res.func_id("make").expect("`make` is resolved");
    assert_eq!(res.func_def(id).name, "make");
}

#[test]
fn parameters_are_bindings_of_their_function() {
    let (program, res, diags) = resolve("fn f(a: Int, b: Int) { let c = a; }\nfn main() { }");
    assert!(diags.is_empty(), "{diags:?}");
    let Item::Function(f) = &program.items[0] else {
        panic!("expected a function");
    };
    // The declaration site and the use site agree on the binding.
    let declared = res
        .binding(f.params[0].name.id)
        .expect("a parameter introduces a binding");
    let Stmt::Let { value, .. } = &f.body.stmts[0] else {
        panic!("expected a let");
    };
    assert_eq!(res.resolution(value.id), Resolution::Local(declared));
}

#[test]
fn unresolved_names_resolve_to_error_exactly_once() {
    let (program, res, diags) = resolve("fn main() { let x = missing; }");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "E0201");
    let refs = var_refs(&program);
    assert_eq!(res.resolution(refs[0].1), Resolution::Error);
}

#[test]
fn a_call_resolves_to_the_function_it_names() {
    let (program, res, diags) =
        resolve("fn add(a: Int) -> Int { return a; }\nfn main() { add(1); }");
    assert!(diags.is_empty(), "{diags:?}");
    let Item::Function(main) = &program.items[1] else {
        panic!("expected main");
    };
    let Stmt::Expr(call) = &main.body.stmts[0] else {
        panic!("expected an expression statement");
    };
    let ExprKind::Call { callee, .. } = &call.kind else {
        panic!("expected a call");
    };
    let expected = res.func_id("add").unwrap();
    assert_eq!(res.resolution(callee.id), Resolution::Function(expected));
}
