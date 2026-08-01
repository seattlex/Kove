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
    // The parameter is also unused, so look for the error specifically
    // rather than assuming it is the only diagnostic.
    for src in ["fn main(x: Int) { }", "fn main() -> Int { return 1; }"] {
        let c = kove_cli::compile_executable("t.kov", src);
        let errors: Vec<&str> = c
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.code)
            .collect();
        assert_eq!(errors, vec!["E0214"], "for {src:?}");
    }
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
fn compound_assignment_runs() {
    let out = run("struct C { hits: Int }\n\
         fn main() {\n\
             let mut n = 10;\n\
             n += 5;\n\
             n -= 3;\n\
             n *= 2;\n\
             n /= 4;\n\
             n %= 4;\n\
             println(n);\n\
             let mut c = C { hits: 0 };\n\
             c.hits += 7;\n\
             println(c.hits);\n\
         }");
    assert_eq!(out, "2\n7\n");
}

#[test]
fn compound_assignment_is_checked_like_any_arithmetic() {
    // The desugaring inherits the overflow and divide-by-zero checks.
    assert_eq!(
        run_expecting_runtime_error("fn main() { let mut x = 9223372036854775807; x += 1; }"),
        "E0302"
    );
    assert_eq!(
        run_expecting_runtime_error("fn main() { let z = 0; let mut x = 1; x /= z; }"),
        "E0301"
    );
}

#[test]
fn conversions_run() {
    assert_eq!(
        run("fn main() { println(to_float(3)); println(to_float(0 - 3)); }"),
        "3\n-3\n"
    );
    // Truncation is toward zero, in both directions.
    assert_eq!(
        run("fn main() { println(to_int(2.9)); println(to_int(0.0 - 2.9)); }"),
        "2\n-2\n"
    );
    // An Int mean of Ints, done properly.
    assert_eq!(
        run("fn main() { println(to_float(7) / to_float(2)); }"),
        "3.5\n"
    );
}

#[test]
fn char_ordering_runs() {
    assert_eq!(
        run("fn main() { println('a' < 'b'); println('b' < 'a'); println('a' <= 'a'); }"),
        "true\nfalse\ntrue\n"
    );
    // Ordering is by Unicode scalar value, so it does not stop at ASCII.
    assert_eq!(run("fn main() { println('a' < 'é'); }"), "true\n");
    // The case this was added for.
    assert_eq!(
        run(
            "fn classify(c: Char) -> Bool { return c >= '0' && c <= '9'; }\n\
             fn main() { println(classify('7')); println(classify('/')); println(classify(':')); }"
        ),
        "true\nfalse\nfalse\n"
    );
}

#[test]
fn e0307_float_that_no_int_can_stand_for() {
    for src in [
        "fn main() { println(to_int(1.0 / 0.0)); }",
        "fn main() { println(to_int(0.0 - 1.0 / 0.0)); }",
        "fn main() { println(to_int(0.0 / 0.0)); }",
        "fn main() { println(to_int(100000000000000000000.0)); }",
    ] {
        assert_eq!(run_expecting_runtime_error(src), "E0307", "for {src}");
    }
}

#[test]
fn e0306_failed_assertion() {
    assert_eq!(
        run_expecting_runtime_error("fn main() { assert(1 == 2); }"),
        "E0306"
    );
    // A passing assertion is invisible.
    assert_eq!(run("fn main() { assert(true); println(1); }"), "1\n");
}

#[test]
fn float_division_by_zero_is_not_an_error() {
    // IEEE semantics for Float, checked semantics for Int. Documented.
    assert_eq!(run("fn main() { println(1.0 / 0.0); }"), "inf\n");
}
