//! Type checker tests: one test per diagnostic code plus programs that
//! must check cleanly. The frontend is exercised end to end, so these are
//! also name-resolution tests.

use kove_tests::codes;

#[track_caller]
fn assert_ok(src: &str) {
    assert_eq!(codes(src), Vec::<&str>::new(), "expected no errors for {src:?}");
}

#[track_caller]
fn assert_code(src: &str, code: &str) {
    let found = codes(src);
    assert!(
        found.contains(&code),
        "expected {code} for {src:?}, got {found:?}"
    );
}

// --- Programs that must check cleanly --------------------------------------

#[test]
fn valid_programs() {
    assert_ok("fn main() { }");
    assert_ok("fn add(a: Int, b: Int) -> Int { return a + b; }\nfn main() { let x = add(1, 2); println(x); }");
    assert_ok("fn main() { let mut n = 0; n = n + 1; while n < 3 { n = n + 1; } }");
    assert_ok("fn main() { for i in 0..10 { println(i); } }");
    assert_ok("struct P { x: Int, y: Int }\nfn main() { let mut p = P { x: 1, y: 2 }; p.x = 3; println(p.x); }");
    assert_ok("enum S { A, B }\nfn main() { let s = S::A; if s == S::B { } }");
    assert_ok("fn main() { let a = 1.5; let b = a * 2.0; println(b); }");
    assert_ok("fn main() { let x = 5; { let x = true; if x { } } println(x); }"); // shadowing in scopes
    assert_ok("fn f(c: Char) -> Bool { return c == 'k'; }\nfn main() { println(f('k')); }");
}

#[test]
fn structs_and_enums_can_be_referenced_before_declaration() {
    assert_ok("fn make() -> P { return P { x: 1 }; }\nstruct P { x: Int }\nfn main() { println(make().x); }");
}

// --- One test per code ------------------------------------------------------

#[test]
fn e0012_type_mismatch() {
    assert_code("fn main() { let age: Int = \"sixteen\"; }", "E0012");
    assert_code("fn f() -> Int { return true; }\nfn main() { }", "E0012");
    assert_code("fn f(a: Int) { }\nfn main() { f(\"x\"); }", "E0012");
    assert_code("fn main() { let mut x = 1; x = \"s\"; }", "E0012");
    assert_code("fn main() { for i in 0..\"x\" { } }", "E0012");
}

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
fn e0203_wrong_argument_count() {
    assert_code("fn f(a: Int) { }\nfn main() { f(1, 2); }", "E0203");
    assert_code("fn main() { println(1, 2); }", "E0203");
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
fn e0206_unknown_field() {
    assert_code(
        "struct P { x: Int }\nfn main() { let p = P { x: 1 }; println(p.y); }",
        "E0206",
    );
    assert_code("struct P { x: Int }\nfn main() { let p = P { x: 1, y: 2 }; }", "E0206");
}

#[test]
fn e0207_missing_field() {
    assert_code("struct P { x: Int, y: Int }\nfn main() { let p = P { x: 1 }; }", "E0207");
}

#[test]
fn e0208_duplicate_field_initializer() {
    assert_code("struct P { x: Int }\nfn main() { let p = P { x: 1, x: 2 }; }", "E0208");
}

#[test]
fn e0209_field_access_on_non_struct() {
    assert_code("fn main() { let n = 1; println(n.field); }", "E0209");
}

#[test]
fn e0210_missing_return() {
    assert_code("fn f() -> Int { }\nfn main() { }", "E0210");
    assert_code(
        "fn f(x: Int) -> Int { if x > 0 { return 1; } }\nfn main() { }",
        "E0210",
    );
    // While bodies may never run, so they do not count.
    assert_code(
        "fn f() -> Int { while true { return 1; } }\nfn main() { }",
        "E0210",
    );
    // But exhaustive if/else does.
    assert_ok("fn f(x: Int) -> Int { if x > 0 { return 1; } else { return 2; } }\nfn main() { }");
}

#[test]
fn e0211_condition_must_be_bool() {
    assert_code("fn main() { if 1 { } }", "E0211");
    assert_code("fn main() { while \"s\" { } }", "E0211");
}

#[test]
fn e0212_invalid_operands() {
    assert_code("fn main() { let x = 1 + \"s\"; }", "E0212");
    assert_code("fn main() { let x = 1 + 1.5; }", "E0212"); // no implicit conversion
    assert_code("fn main() { let x = -true; }", "E0212");
    assert_code("fn main() { let x = !1; }", "E0212");
    assert_code("fn main() { let x = 1 && true; }", "E0212");
    assert_code(
        "struct P { x: Int }\nfn main() { let a = P { x: 1 }; let b = P { x: 1 }; let e = a == b; }",
        "E0212",
    );
}

#[test]
fn e0213_invalid_assignment_target() {
    assert_code("fn f() -> Int { return 1; }\nfn main() { f() = 2; }", "E0213");
}

#[test]
fn e0215_unprintable_value() {
    assert_code(
        "struct P { x: Int }\nfn main() { println(P { x: 1 }); }",
        "E0215",
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
fn e0218_for_needs_a_range() {
    assert_code("fn main() { for x in 5 { } }", "E0218");
    assert_code("fn main() { for x in \"abc\" { } }", "E0218");
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

// --- Error-recovery quality -------------------------------------------------

#[test]
fn one_mistake_one_error() {
    // The undefined variable is reported once; the uses of the resulting
    // error type do not cascade.
    let found = codes("fn main() { let x = missing; let y = x + 1; println(y); }");
    assert_eq!(found, vec!["E0201"]);
}

#[test]
fn multiple_independent_errors_are_all_reported() {
    let found = codes(
        "fn main() { let a: Int = \"s\"; let b: Bool = 1; let c = missing; }",
    );
    assert_eq!(found, vec!["E0012", "E0012", "E0201"]);
}

#[test]
fn syntax_errors_suppress_type_checking() {
    // With a broken parse we do not pile semantic errors on top.
    let found = codes("fn main() { let x: Int = \"s\" }"); // missing `;`
    assert!(found.iter().all(|c| c.starts_with("E01")), "got {found:?}");
}
