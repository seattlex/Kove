//! Resolver tests: what each name refers to, and the diagnostics about
//! names. Types are `typecheck.rs`.
//!
//! The `assert_code` tests run the whole driver, so they also confirm the
//! resolver's diagnostics reach the user; the tests at the bottom go
//! through the resolver API directly to check the resolution map itself.

use kove_ast::{ExprKind, Item, Stmt};
use kove_diagnostics::Diagnostic;
use kove_resolver::Resolution;
use kove_tests::{codes, resolve};

/// These tests are about the resolution map, so they check for errors and
/// let warnings (such as the unused-binding lint) be.
#[track_caller]
fn assert_no_errors(diags: &[Diagnostic]) {
    let errors: Vec<&Diagnostic> = diags.iter().filter(|d| d.is_error()).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

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

#[test]
fn builtin_names_are_reserved() {
    assert_code("fn println(x: Int) { }\nfn main() { }", "E0205");
    assert_code("fn assert(c: Bool) { }\nfn main() { }", "E0205");
}

// --- Suggestions -----------------------------------------------------------

/// The `help:` texts of a program's diagnostics.
fn helps(src: &str) -> Vec<String> {
    kove_cli::compile("test.kov", src)
        .diagnostics
        .iter()
        .filter_map(|d| d.help.clone())
        .collect()
}

#[test]
fn a_mistyped_variable_suggests_one_in_scope() {
    let helps = helps("fn main() { let length = 3; println(lenght); }");
    assert!(
        helps.iter().any(|h| h == "did you mean `length`?"),
        "{helps:?}"
    );
}

#[test]
fn a_mistyped_type_suggests_a_known_one() {
    assert!(helps("fn main() { let x: Strng = \"s\"; }")
        .iter()
        .any(|h| h == "did you mean `String`?"));
    assert!(
        helps("struct Point { x: Int }\nfn f(p: Pont) { }\nfn main() { }")
            .iter()
            .any(|h| h == "did you mean `Point`?")
    );
}

#[test]
fn a_mistyped_function_suggests_a_declared_one_or_a_builtin() {
    assert!(helps("fn distance() { }\nfn main() { distence(); }")
        .iter()
        .any(|h| h == "did you mean `distance`?"));
    assert!(helps("fn main() { prntln(1); }")
        .iter()
        .any(|h| h == "did you mean `println`?"));
}

#[test]
fn nothing_is_suggested_when_nothing_is_close() {
    // A wrong guess is worse than no guess.
    let helps = helps("fn main() { let length = 3; println(banana); }");
    assert!(
        !helps.iter().any(|h| h.starts_with("did you mean")),
        "{helps:?}"
    );
}

// --- Lints -----------------------------------------------------------------

#[test]
fn w0001_unused_bindings() {
    use kove_tests::warning_codes;

    assert_eq!(
        warning_codes("fn main() { let x = 1; }"),
        vec!["W0001"],
        "an unused let warns"
    );
    assert_eq!(
        warning_codes("fn f(a: Int) { }\nfn main() { f(1); }"),
        vec!["W0001"],
        "an unused parameter warns"
    );
    assert_eq!(
        warning_codes("fn main() { for i in 0..3 { } }"),
        vec!["W0001"],
        "an unused loop variable warns"
    );
}

#[test]
fn w0001_stays_quiet_when_a_binding_is_used() {
    use kove_tests::warning_codes;

    assert!(warning_codes("fn main() { let x = 1; println(x); }").is_empty());
    assert!(warning_codes("fn f(a: Int) -> Int { return a; }\nfn main() { f(1); }").is_empty());
    assert!(warning_codes("fn main() { for i in 0..3 { println(i); } }").is_empty());
    // Shadowing: both bindings are read.
    assert!(
        warning_codes("fn main() { let x = 1; println(x); let x = 2; println(x); }").is_empty()
    );
}

#[test]
fn an_underscore_prefix_silences_the_unused_lint() {
    use kove_tests::warning_codes;

    assert!(warning_codes("fn main() { let _x = 1; }").is_empty());
    assert!(warning_codes("fn f(_a: Int) { }\nfn main() { f(1); }").is_empty());
}

#[test]
fn warnings_do_not_stop_compilation() {
    // The program still compiles and runs with an unused binding in it.
    let c = kove_cli::compile_executable("t.kov", "fn main() { let x = 1; println(2); }");
    assert!(!c.has_errors());
    assert_eq!(c.warning_count(), 1);
}

#[test]
fn w0001_catches_a_variable_that_is_only_written() {
    use kove_tests::warning_codes;

    // Writing is not reading.
    assert_eq!(
        warning_codes("fn main() { let mut x = 0; x = 1; }"),
        vec!["W0001"]
    );
    // ...but a later read clears it.
    assert!(warning_codes("fn main() { let mut x = 0; x = 1; println(x); }").is_empty());
    // Assigning through a field reads the root, so it counts as a use.
    assert!(
        warning_codes("struct B { v: Int }\nfn main() { let mut b = B { v: 1 }; b.v = 2; }")
            .is_empty()
    );
}

#[test]
fn a_shadowed_binding_that_is_never_read_still_warns() {
    use kove_tests::warning_codes;

    // The first `x` is shadowed before anything reads it.
    assert_eq!(
        warning_codes("fn main() { let x = 1; let x = 2; println(x); }"),
        vec!["W0001"]
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
    assert_no_errors(&diags);
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
    assert_no_errors(&diags);
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
    assert_no_errors(&diags);
    let id = res.func_id("make").expect("`make` is resolved");
    assert_eq!(res.func_def(id).name, "make");
}

#[test]
fn parameters_are_bindings_of_their_function() {
    let (program, res, diags) = resolve("fn f(a: Int, b: Int) { let c = a; }\nfn main() { }");
    assert_no_errors(&diags);
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
    let errors: Vec<&str> = diags
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.code)
        .collect();
    assert_eq!(errors, vec!["E0201"]);
    let refs = var_refs(&program);
    assert_eq!(res.resolution(refs[0].1), Resolution::Error);
}

#[test]
fn a_call_resolves_to_the_function_it_names() {
    let (program, res, diags) =
        resolve("fn add(a: Int) -> Int { return a; }\nfn main() { add(1); }");
    assert_no_errors(&diags);
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
