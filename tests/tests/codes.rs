//! The diagnostic code registry, and whether it agrees with the
//! documentation.
//!
//! Codes are meant to be stable enough that documentation and search
//! results can point at them, which only holds if the registry, the
//! documentation and the compiler say the same thing. These tests make
//! that mechanical instead of a promise.

use kove_diagnostics::{explain, CODES};
use std::collections::HashSet;

/// Codes listed in the tables in `docs/diagnostics.md`.
fn documented_codes() -> HashSet<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("docs/diagnostics.md");
    let text = std::fs::read_to_string(path).expect("docs/diagnostics.md is readable");
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("| ")?;
            let code = rest.split(" |").next()?;
            let mut chars = code.chars();
            let first = chars.next()?;
            if (first == 'E' || first == 'W') && chars.all(|c| c.is_ascii_digit()) {
                Some(code.to_string())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn every_documented_code_has_an_explanation() {
    let registry: HashSet<String> = CODES.iter().map(|c| c.code.to_string()).collect();
    let documented = documented_codes();
    assert!(!documented.is_empty(), "no codes found in the docs");

    let missing: Vec<&String> = documented.difference(&registry).collect();
    assert!(
        missing.is_empty(),
        "documented but not in the registry: {missing:?}"
    );
}

#[test]
fn every_explained_code_is_documented() {
    let registry: HashSet<String> = CODES.iter().map(|c| c.code.to_string()).collect();
    let documented = documented_codes();

    let extra: Vec<&String> = registry.difference(&documented).collect();
    assert!(
        extra.is_empty(),
        "in the registry but not documented: {extra:?}"
    );
}

#[test]
fn codes_are_unique() {
    let mut seen = HashSet::new();
    for info in CODES {
        assert!(seen.insert(info.code), "duplicate code {}", info.code);
    }
}

#[test]
fn lookup_ignores_case_and_rejects_the_unknown() {
    assert_eq!(explain("E0012").unwrap().code, "E0012");
    assert_eq!(explain("e0012").unwrap().code, "E0012");
    assert!(explain("E9999").is_none());
    assert!(explain("").is_none());
    assert!(explain("nonsense").is_none());
}

#[test]
fn every_entry_is_filled_in() {
    for info in CODES {
        assert!(!info.summary.is_empty(), "{} has no summary", info.code);
        assert!(
            !info.explanation.is_empty(),
            "{} has no explanation",
            info.code
        );
        // A summary is a label, not a sentence.
        assert!(
            !info.summary.ends_with('.'),
            "{} summary should not end with a period",
            info.code
        );
        assert!(
            info.explanation.lines().count() >= 2,
            "{} explanation is too thin to be worth printing",
            info.code
        );
    }
}

/// Codes the compiler actually emits for a set of broken programs, to
/// catch a code being emitted that nobody registered.
#[test]
fn codes_emitted_by_the_compiler_are_all_registered() {
    let programs = [
        "fn main() { let x = 1 @ 2; }",
        "fn main() { let x = 1 }",
        "let x = 1;",
        "fn main() { let x = 99999999999999999999; }",
        r#"fn main() { let s = "a\qb"; }"#,
        "fn main() { let s = \"oops;\n}",
        "fn main() { let c = 'a;\n}",
        "fn main() { } /* never closed",
        "fn main() { let age: Int = \"sixteen\"; }",
        "fn main() { let x: Nope = 1; }",
        "fn main() { println(missing); }",
        "fn main() { nope(); }",
        "fn f(a: Int) { }\nfn main() { f(1, 2); }",
        "fn main() { let a = 1; a = 2; }",
        "fn f() { }\nfn f() { }\nfn main() { }",
        "struct P { x: Int }\nfn main() { let p = P { x: 1 }; println(p.y); }",
        "struct P { x: Int, y: Int }\nfn main() { let p = P { x: 1 }; }",
        "struct P { x: Int }\nfn main() { let p = P { x: 1, x: 2 }; }",
        "fn main() { let n = 1; println(n.f); }",
        "fn f() -> Int { }\nfn main() { }",
        "fn main() { if 1 { } }",
        "fn main() { let x = 1 + true; }",
        "fn f() -> Int { return 1; }\nfn main() { f() = 2; }",
        "struct P { x: Int }\nfn main() { println(P { x: 1 }); }",
        "enum S { A }\nfn main() { let s = S::B; }",
        "import std::io;\nfn main() { }",
        "fn main() { for x in 5 { } }",
        "enum S { A }\nfn main() { let s = S { a: 1 }; }",
        "struct P { x: Int }\nfn main() { let p = P { x: 1 }; p.x(); }",
        "fn main() { let unused = 1; }",
        "fn dead() { }\nfn main() { }",
    ];
    for src in programs {
        for code in kove_tests::all_codes(src) {
            assert!(
                explain(code).is_some(),
                "{code} is emitted for {src:?} but has no registry entry"
            );
        }
    }
}
