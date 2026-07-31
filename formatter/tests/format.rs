//! Formatter tests.
//!
//! Three kinds. Golden tests pin the canonical output for each construct.
//! The property tests are the ones that matter most: formatting must be
//! idempotent, and it must never change what the code means.

use kove_formatter::format;

#[track_caller]
fn assert_formats(input: &str, expected: &str) {
    let got = format(input).expect("input parses");
    assert_eq!(got, expected, "\ninput:\n{input}");
    // Whatever the input looked like, the output is a fixed point.
    let again = format(&got).expect("output parses");
    assert_eq!(again, got, "formatting is not idempotent for {input:?}");
}

/// Significant tokens, ignoring whitespace and comments. Formatting may
/// move these around but must never add, drop or alter one.
///
/// The single exception is a trailing separator: the comma in
/// `struct S { a: Int, }` means nothing, and the formatter normalizes it
/// away. It is dropped from both sides here so the comparison stays a
/// real check on everything else.
fn tokens(source: &str) -> Vec<String> {
    let doc = kove_parser::parse(source);
    let lang = doc.language().clone();
    let mut out = Vec::new();
    for elem in doc.tree().root().descendants() {
        if let reparse::SyntaxElem::Token(t) = elem {
            if t.kind() == reparse::grammar::EOF_TOKEN || t.is_missing() {
                continue;
            }
            out.push(format!(
                "{}:{}",
                lang.token_name(t.kind()),
                t.text(doc.text())
            ));
        }
    }
    drop_trailing_separators(out)
}

fn drop_trailing_separators(tokens: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    for (i, t) in tokens.iter().enumerate() {
        let closes_next = matches!(
            tokens.get(i + 1).map(String::as_str),
            Some("}:}") | Some("):)")
        );
        if t == ",:," && closes_next {
            continue;
        }
        out.push(t.clone());
    }
    out
}

// --- Golden output ---------------------------------------------------------

#[test]
fn functions() {
    assert_formats(
        "fn   add(a:Int,b:Int)->Int{return a+b;}",
        "fn add(a: Int, b: Int) -> Int {\n    return a + b;\n}\n",
    );
    assert_formats("fn main(){}", "fn main() {}\n");
}

#[test]
fn statements() {
    assert_formats(
        "fn f(){let  mut x:Int=1;x=x+1;}",
        "fn f() {\n    let mut x: Int = 1;\n    x = x + 1;\n}\n",
    );
}

#[test]
fn control_flow() {
    assert_formats(
        "fn f(x:Int){if x>0{println(1);}else if x<0{println(2);}else{println(3);}}",
        "fn f(x: Int) {\n    \
           if x > 0 {\n        println(1);\n    } \
           else if x < 0 {\n        println(2);\n    } \
           else {\n        println(3);\n    }\n}\n",
    );
    assert_formats(
        "fn f(){while true{}for i in 0..10{}}",
        "fn f() {\n    while true {}\n    for i in 0..10 {}\n}\n",
    );
}

#[test]
fn ranges_have_no_spaces() {
    assert_formats(
        "fn f(){for i in 0 .. 10{}}",
        "fn f() {\n    for i in 0..10 {}\n}\n",
    );
}

#[test]
fn declarations_put_one_member_per_line() {
    assert_formats(
        "struct User{name:String,age:Int}",
        "struct User {\n    name: String,\n    age: Int\n}\n",
    );
    assert_formats("enum S{A,B}", "enum S {\n    A,\n    B\n}\n");
    // A trailing comma in the source does not survive.
    assert_formats("enum S{A,B,}", "enum S {\n    A,\n    B\n}\n");
}

#[test]
fn expressions() {
    assert_formats(
        "fn f(){let x=-a+ b*(c-d)/e%f;let y=!p&&q||r;}",
        "fn f() {\n    let x = -a + b * (c - d) / e % f;\n    let y = !p && q || r;\n}\n",
    );
    assert_formats(
        "fn f(){g(  1,2 );let n=s.field.inner;let v=E::V;}",
        "fn f() {\n    g(1, 2);\n    let n = s.field.inner;\n    let v = E::V;\n}\n",
    );
}

#[test]
fn redundant_parentheses_are_preserved() {
    // Removing them would mean reasoning about precedence.
    assert_formats(
        "fn f(){let x=(1+2);}",
        "fn f() {\n    let x = (1 + 2);\n}\n",
    );
}

#[test]
fn struct_literals_break_only_when_they_must() {
    assert_formats(
        "struct P{x:Int}\nfn f(){let p=P{\n x:1\n};}",
        "struct P {\n    x: Int\n}\nfn f() {\n    let p = P { x: 1 };\n}\n",
    );
    // Too wide for one line, so one field per line.
    let wide = format!(
        "struct P {{ a: String }}\nfn f() {{ let p = P {{ a: \"{}\" }}; }}",
        "x".repeat(100)
    );
    let out = format(&wide).unwrap();
    assert!(out.contains("let p = P {\n        a: "), "{out}");
    assert_eq!(format(&out).unwrap(), out, "not idempotent");
}

#[test]
fn imports() {
    assert_formats("import  std :: io ;", "import std::io;\n");
}

// --- Comments --------------------------------------------------------------

#[test]
fn a_comment_on_its_own_line_stays_there() {
    assert_formats(
        "// about the function\nfn main(){\n// about the statement\nlet x=1;\n}",
        "// about the function\nfn main() {\n    // about the statement\n    let x = 1;\n}\n",
    );
}

#[test]
fn a_trailing_comment_stays_on_its_line() {
    assert_formats(
        "fn main(){let x=1; // why one\n}",
        "fn main() {\n    let x = 1; // why one\n}\n",
    );
}

#[test]
fn an_inline_block_comment_stays_inline() {
    assert_formats(
        "fn main(){let x=/* why */1;}",
        "fn main() {\n    let x = /* why */ 1;\n}\n",
    );
}

#[test]
fn comments_survive_in_empty_bodies_and_at_end_of_file() {
    assert_formats(
        "fn main(){\n// nothing yet\n}\n// the end",
        "fn main() {\n    // nothing yet\n}\n// the end\n",
    );
}

#[test]
fn comments_in_declarations_survive() {
    assert_formats(
        "struct P{\n// the x\nx:Int,\ny:Int // the y\n}",
        "struct P {\n    // the x\n    x: Int,\n    y: Int // the y\n}\n",
    );
}

// --- Blank lines -----------------------------------------------------------

#[test]
fn one_blank_line_is_kept_and_more_are_collapsed() {
    assert_formats("fn f(){}\n\n\n\nfn g(){}", "fn f() {}\n\nfn g() {}\n");
    assert_formats(
        "fn f(){let a=1;\n\n\nlet b=2;}",
        "fn f() {\n    let a = 1;\n\n    let b = 2;\n}\n",
    );
}

#[test]
fn no_blank_line_stays_no_blank_line() {
    assert_formats("fn f(){}\nfn g(){}", "fn f() {}\nfn g() {}\n");
}

// --- Properties ------------------------------------------------------------

/// Every Kove program in the repository, as formatter input.
fn repository_programs() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let mut out = Vec::new();
    let mut dirs = vec![root.join("examples"), root.join("tests/programs")];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "kov") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    out.push((path.display().to_string(), text));
                }
            }
        }
    }
    assert!(!out.is_empty(), "no .kov files found");
    out
}

#[test]
fn formatting_is_idempotent_across_the_repository() {
    for (name, text) in repository_programs() {
        let Ok(once) = format(&text) else {
            continue; // fixtures that intentionally do not parse
        };
        let twice = format(&once).expect("formatted output parses");
        assert_eq!(once, twice, "not idempotent: {name}");
    }
}

#[test]
fn formatting_never_changes_the_token_stream() {
    // The strongest guarantee the formatter offers: it moves whitespace
    // and nothing else.
    for (name, text) in repository_programs() {
        let Ok(formatted) = format(&text) else {
            continue;
        };
        assert_eq!(
            tokens(&text),
            tokens(&formatted),
            "formatting changed the tokens of {name}"
        );
    }
}

#[test]
fn formatting_never_changes_the_token_stream_for_awkward_input() {
    for src in [
        "fn f(){let x=1;}",
        "fn f ( a : Int ) -> Int { return a ; }",
        "struct S{a:Int,b:Int,}",
        "enum E{A}",
        "fn f(){if a{}else if b{}else{}}",
        "fn f(){let s=\"a\\tb\";let c='k';let n=1.5;}",
        "fn f(){g(h(i(1)),2);}",
        "fn f(){let p=P{x:1,y:2};}",
        "import a::b::c;",
        "fn f(){for i in 0..n{while x{y=y+1;}}}",
        "// only a comment\n",
    ] {
        let formatted = format(src).unwrap_or_else(|e| panic!("{src:?} failed: {e:?}"));
        assert_eq!(
            tokens(src),
            tokens(&formatted),
            "tokens changed for {src:?}"
        );
        assert_eq!(
            format(&formatted).unwrap(),
            formatted,
            "not idempotent: {src:?}"
        );
    }
}

#[test]
fn output_always_ends_with_exactly_one_newline() {
    for src in ["fn f(){}", "fn f(){}\n\n\n", "// comment only"] {
        let out = format(src).unwrap();
        assert!(out.ends_with('\n'), "{out:?}");
        assert!(!out.ends_with("\n\n"), "{out:?}");
    }
}

#[test]
fn an_empty_file_stays_empty() {
    assert_eq!(format("").unwrap(), "");
    assert_eq!(format("   \n\n").unwrap(), "");
}

#[test]
fn broken_input_is_refused_rather_than_rewritten() {
    let err = format("fn main() { let x = 1 }").unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(err[0].code, "E0101");
}
