//! Diagnostic rendering tests: the exact output format is part of Kove's
//! contract (documented in `docs/diagnostics.md`), so it is golden-tested.

use kove_diagnostics::{render, render_all, Diagnostic, SourceFile, Span};

#[test]
fn the_reference_example_renders_exactly() {
    // The canonical example from the requirements document.
    let src = "fn main() {\n    let age: Int = \"sixteen\";\n    println(age);\n}\n";
    let c = kove_cli::compile("src/main.kov", src);
    assert_eq!(c.diagnostics.len(), 1);
    let out = render_all(&c.diagnostics, &c.source);
    assert_eq!(
        out,
        "error[E0012]: mismatched types: expected `Int`, found `String`\n \
         --> src/main.kov:2:20\n  \
         |\n\
         2 |     let age: Int = \"sixteen\";\n  \
         |                    ^^^^^^^^^ expected `Int`\n  \
         |\n\
         help: remove the quotes or change the variable type\n"
    );
}

#[test]
fn line_and_column_are_one_based_and_counted_in_characters() {
    let file = SourceFile::new("t.kov", "abc\ndef\n");
    assert_eq!(file.line_col(0), (1, 1));
    assert_eq!(file.line_col(4), (2, 1));
    assert_eq!(file.line_col(6), (2, 3));
}

#[test]
fn multi_line_spans_underline_the_first_line() {
    let file = SourceFile::new("t.kov", "first line\nsecond line\n");
    let d = Diagnostic::error("E0000", "test", Span::new(6, 18));
    let out = render(&d, &file);
    assert!(out.contains("1 | first line\n"), "{out}");
    assert!(out.contains("|       ^^^^"), "{out}");
    assert!(
        !out.contains("second"),
        "only the first line is shown: {out}"
    );
}

#[test]
fn empty_spans_render_one_caret() {
    let file = SourceFile::new("t.kov", "let x = 1\n");
    let d = Diagnostic::error("E0101", "expected `;`", Span::empty(9));
    let out = render(&d, &file);
    assert!(out.contains(&format!("| {}^", " ".repeat(9))), "{out}");
    assert!(out.contains("--> t.kov:1:10"), "{out}");
}

#[test]
fn tabs_align_with_the_carets() {
    let file = SourceFile::new("t.kov", "\tbad\n");
    let d = Diagnostic::error("E0000", "test", Span::new(1, 4));
    let out = render(&d, &file);
    // The tab renders as four spaces, and the carets start after them.
    assert!(out.contains("1 |     bad\n"), "{out}");
    assert!(out.contains("|     ^^^"), "{out}");
}

#[test]
fn notes_and_help_render_after_the_snippet() {
    let file = SourceFile::new("t.kov", "x\n");
    let d = Diagnostic::error("E0000", "message", Span::new(0, 1))
        .with_label("label")
        .with_help("try this")
        .with_note("background");
    let out = render(&d, &file);
    assert!(out.contains("^ label\n"), "{out}");
    assert!(out.contains("help: try this\n"), "{out}");
    assert!(out.contains("note: background\n"), "{out}");
    // help comes before notes, both after the underline.
    let help_at = out.find("help:").unwrap();
    let note_at = out.find("note:").unwrap();
    assert!(help_at < note_at);
}

#[test]
fn diagnostics_render_in_source_order() {
    let src = "fn main() { let a: Int = \"s\"; let b: Bool = 1; }";
    let c = kove_cli::compile("t.kov", src);
    let out = render_all(&c.diagnostics, &c.source);
    let first = out.find("expected `Int`").unwrap();
    let second = out.find("expected `Bool`").unwrap();
    assert!(first < second);
}

#[test]
fn spans_at_end_of_file_render() {
    // A missing `}` at EOF must not panic the renderer.
    let c = kove_cli::compile("t.kov", "fn main() {");
    assert!(!c.diagnostics.is_empty());
    let out = render_all(&c.diagnostics, &c.source);
    assert!(out.contains("--> t.kov:1:12"), "{out}");
}
