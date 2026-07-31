//! Token-level tests: keywords vs identifiers, literals, longest-match
//! operators, comments as trivia, and lex-stage diagnostics.

use kove_tests::{codes, tokens};

fn kinds(text: &str) -> Vec<String> {
    tokens(text).into_iter().map(|(k, _)| k).collect()
}

#[test]
fn keywords_and_identifiers() {
    assert_eq!(
        kinds("fn main() { let mutable = 0; }"),
        vec![
            "fn",
            "identifier",
            "(",
            ")",
            "{",
            "let",
            "identifier",
            "=",
            "int",
            ";",
            "}"
        ]
    );
    // A keyword is only a keyword when it matches exactly.
    let toks = tokens("fn f() { let letter = 1; let iff = 2; }");
    assert!(toks.iter().any(|(k, t)| k == "identifier" && t == "letter"));
    assert!(toks.iter().any(|(k, t)| k == "identifier" && t == "iff"));
}

#[test]
fn numbers_and_ranges() {
    // `1.5` is one float; `1..10` is int, `..`, int.
    let toks = tokens("fn f() { let a = 1.5; let b = 1..10; }");
    assert!(toks.contains(&("float".into(), "1.5".into())));
    assert!(toks.contains(&("..".into(), "..".into())));
    assert!(toks.contains(&("int".into(), "1".into())));
    assert!(toks.contains(&("int".into(), "10".into())));
}

#[test]
fn longest_match_operators() {
    let toks = tokens("fn f() -> Int { return 1 == 2 != 3 <= 4 >= 5; }");
    let ops: Vec<&str> = toks
        .iter()
        .filter(|(k, _)| ["==", "!=", "<=", ">=", "->"].contains(&k.as_str()))
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(ops, vec!["->", "==", "!=", "<=", ">="]);
}

#[test]
fn path_separator_is_not_two_colons() {
    let toks = tokens("fn f() { let s = Status::Active; }");
    assert!(toks.contains(&("::".into(), "::".into())));
    assert!(!toks.iter().any(|(k, _)| k == ":"));
}

#[test]
fn comments_are_trivia() {
    // Comments never appear in the significant token stream.
    let toks = kinds("fn f() { /* block */ let x = 1; // line\n }");
    assert!(!toks.iter().any(|k| k.contains("comment")));
    assert!(toks.contains(&"let".to_string()));
}

#[test]
fn string_and_char_literals() {
    let toks = tokens(r#"fn f() { let s = "hi\n"; let c = 'k'; let e = '\n'; }"#);
    assert!(toks.contains(&("string".into(), r#""hi\n""#.into())));
    assert!(toks.contains(&("char".into(), "'k'".into())));
    assert!(toks.contains(&("char".into(), r"'\n'".into())));
}

#[test]
fn unicode_escapes_are_single_char_literals() {
    let toks = tokens(r"fn f() { let c = '\u{1F600}'; }");
    assert!(
        toks.contains(&("char".into(), r"'\u{1F600}'".into())),
        "{toks:?}"
    );
}

#[test]
fn unterminated_string_is_reported() {
    assert!(codes("fn main() { let s = \"oops;\n}").contains(&"E0112"));
}

#[test]
fn unterminated_char_is_reported() {
    assert!(codes("fn main() { let c = 'a;\n}").contains(&"E0113"));
}

#[test]
fn unterminated_block_comment_is_reported() {
    assert!(codes("fn main() { } /* never closed").contains(&"E0114"));
    // A properly closed comment is fine.
    assert_eq!(codes("fn main() { } /* closed */"), Vec::<&str>::new());
    // `/*/` does not count as closed.
    assert!(codes("fn main() { } /*/").contains(&"E0114"));
}

#[test]
fn unrecognized_character_is_reported() {
    assert!(codes("fn main() { let x = 1 @ 2; }").contains(&"E0001"));
}

#[test]
fn spans_are_exact() {
    // The unterminated string diagnostic points at the string itself.
    let src = "fn main() { let s = \"oops;\n}";
    let c = kove_cli::compile("test.kov", src);
    let diag = c
        .diagnostics
        .iter()
        .find(|d| d.code == "E0112")
        .expect("E0112 reported");
    assert_eq!(
        &src[diag.span.start as usize..diag.span.start as usize + 1],
        "\""
    );
}
