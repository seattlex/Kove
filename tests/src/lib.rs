//! Shared helpers for the Kove test suite. The tests themselves live in
//! `tests/*.rs`, one file per category (lexer, parser, ast, typecheck,
//! diagnostics, compiler, integration).

use std::path::PathBuf;

/// Parse a source text with the Kove grammar.
pub fn parse(text: &str) -> reparse::Document {
    kove_syntax::parse(text)
}

/// The parse tree rendered as an s-expression (for golden tests).
pub fn sexpr(text: &str) -> String {
    let doc = parse(text);
    doc.tree().root().to_sexpr(doc.language(), doc.text())
}

/// The significant tokens of a source text as `(kind, text)` pairs.
/// Trivia and EOF are excluded; missing tokens appear with empty text.
pub fn tokens(text: &str) -> Vec<(String, String)> {
    let doc = parse(text);
    let lang = doc.language().clone();
    let mut out = Vec::new();
    for elem in doc.tree().root().descendants() {
        if let reparse::SyntaxElem::Token(tok) = elem {
            if tok.kind() == reparse::grammar::EOF_TOKEN {
                continue;
            }
            out.push((
                lang.token_name(tok.kind()).to_string(),
                tok.text(doc.text()).to_string(),
            ));
        }
    }
    out
}

/// Diagnostic codes produced by the full frontend, in source order.
pub fn codes(text: &str) -> Vec<&'static str> {
    kove_cli::compile("test.kov", text)
        .diagnostics
        .iter()
        .map(|d| d.code)
        .collect()
}

/// Compile and run a program, returning its stdout. Panics on any
/// compile-time or runtime error.
pub fn run(text: &str) -> String {
    let c = kove_cli::compile_executable("test.kov", text);
    assert!(
        !c.has_errors(),
        "expected a clean compile, got:\n{}",
        kove_diagnostics::render_all(&c.diagnostics, &c.source)
    );
    let mut out = Vec::new();
    kove_cli::run(&c, &mut out).unwrap_or_else(|d| {
        panic!(
            "runtime error:\n{}",
            kove_diagnostics::render(&d, &c.source)
        )
    });
    String::from_utf8(out).expect("program output is UTF-8")
}

/// Compile and run a program, expecting a runtime error; returns its code.
pub fn run_expecting_runtime_error(text: &str) -> &'static str {
    let c = kove_cli::compile_executable("test.kov", text);
    assert!(
        !c.has_errors(),
        "expected a clean compile, got:\n{}",
        kove_diagnostics::render_all(&c.diagnostics, &c.source)
    );
    let mut out = Vec::new();
    match kove_cli::run(&c, &mut out) {
        Ok(()) => panic!("expected a runtime error, but the program finished"),
        Err(d) => d.code,
    }
}

/// Path to the `tests/programs` fixture directory.
pub fn programs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("programs")
}
