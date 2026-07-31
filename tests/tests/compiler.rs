//! Driver tests: phase ordering, the executable checks behind `run` and
//! `build`, and interpreter behavior that is easiest to observe end to end.

use kove_tests::{run, run_expecting_runtime_error};

#[test]
fn check_does_not_require_main() {
    let c = kove_cli::compile("lib.kov", "fn helper() -> Int { return 1; }");
    assert!(!c.has_errors());
}

#[test]
fn executables_require_main() {
    let c = kove_cli::compile_executable("t.kov", "fn helper() -> Int { return 1; }");
    assert_eq!(c.diagnostics.len(), 1);
    assert_eq!(c.diagnostics[0].code, "E0214");
}

#[test]
fn main_must_have_no_parameters_and_no_return_type() {
    let c = kove_cli::compile_executable("t.kov", "fn main(x: Int) { }");
    assert_eq!(c.diagnostics[0].code, "E0214");
    let c = kove_cli::compile_executable("t.kov", "fn main() -> Int { return 1; }");
    assert_eq!(c.diagnostics[0].code, "E0214");
}

#[test]
fn println_prints_every_primitive() {
    let out = run("fn main() {\n\
             println(42);\n\
             println(1.5);\n\
             println(true);\n\
             println('k');\n\
             println(\"text\");\n\
         }");
    assert_eq!(out, "42\n1.5\ntrue\nk\ntext\n");
}

#[test]
fn value_semantics_copy_on_assignment() {
    let out = run("struct P { x: Int }\n\
         fn main() {\n\
             let mut a = P { x: 1 };\n\
             let b = a;\n\
             a.x = 99;\n\
             println(b.x);\n\
         }");
    assert_eq!(out, "1\n");
}

#[test]
fn logical_operators_short_circuit() {
    // If `&&` did not short-circuit, calling `boom` would divide by zero.
    let out = run("fn boom() -> Bool { let x = 1 / 0; return true; }\n\
         fn main() {\n\
             if false && boom() { println(\"no\"); } else { println(\"ok\"); }\n\
             if true || boom() { println(\"ok\"); }\n\
         }");
    assert_eq!(out, "ok\nok\n");
}

#[test]
fn recursion_works() {
    let out = run("fn fib(n: Int) -> Int {\n\
             if n < 2 { return n; }\n\
             return fib(n - 1) + fib(n - 2);\n\
         }\n\
         fn main() { println(fib(15)); }");
    assert_eq!(out, "610\n");
}

#[test]
fn for_ranges_are_half_open() {
    assert_eq!(
        run("fn main() { for i in 0..3 { println(i); } }"),
        "0\n1\n2\n"
    );
    // An empty range runs zero times.
    assert_eq!(run("fn main() { for i in 3..3 { println(i); } }"), "");
    assert_eq!(run("fn main() { for i in 5..3 { println(i); } }"), "");
}

#[test]
fn shadowing_is_per_scope() {
    let out = run("fn main() {\n\
             let x = 1;\n\
             { let x = 2; println(x); }\n\
             println(x);\n\
         }");
    assert_eq!(out, "2\n1\n");
}

// --- Runtime errors ---------------------------------------------------------

#[test]
fn e0301_division_by_zero() {
    assert_eq!(
        run_expecting_runtime_error("fn main() { let z = 0; println(1 / z); }"),
        "E0301"
    );
}

#[test]
fn e0302_integer_overflow() {
    assert_eq!(
        run_expecting_runtime_error("fn main() { let mut x = 9223372036854775807; x = x + 1; }"),
        "E0302"
    );
}

#[test]
fn e0303_remainder_by_zero() {
    assert_eq!(
        run_expecting_runtime_error("fn main() { let z = 0; println(1 % z); }"),
        "E0303"
    );
}

#[test]
fn e0304_recursion_limit() {
    assert_eq!(
        run_expecting_runtime_error(
            "fn forever(n: Int) -> Int { return forever(n + 1); }\n\
             fn main() { println(forever(0)); }"
        ),
        "E0304"
    );
}

#[test]
fn float_division_by_zero_is_not_an_error() {
    // IEEE semantics for Float, checked semantics for Int. Documented.
    assert_eq!(run("fn main() { println(1.0 / 0.0); }"), "inf\n");
}
