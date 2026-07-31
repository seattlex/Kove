//! Machine-readable diagnostics.
//!
//! Editors and CI should not have to scrape the human output, whose
//! layout is tuned for reading and is free to change. This is the stable
//! shape instead: one JSON document per run, with positions given both as
//! 1-based line/column (what people quote) and as byte offsets (what
//! tools index with).
//!
//! Written by hand rather than with a serialization crate, in keeping
//! with the rest of the toolchain's dependencies.

use crate::{Diagnostic, SourceFile};

/// Render diagnostics as a JSON document:
///
/// ```json
/// {
///   "file": "src/main.kov",
///   "diagnostics": [
///     {
///       "severity": "error",
///       "code": "E0012",
///       "message": "mismatched types: expected `Int`, found `String`",
///       "start": { "line": 2, "column": 20, "offset": 34 },
///       "end":   { "line": 2, "column": 29, "offset": 43 },
///       "label": "expected `Int`",
///       "help": "remove the quotes or change the variable type",
///       "notes": []
///     }
///   ]
/// }
/// ```
///
/// `label` and `help` are `null` when absent rather than omitted, so a
/// consumer can read the same keys every time.
pub fn render_json(diagnostics: &[Diagnostic], file: &SourceFile) -> String {
    let mut sorted: Vec<&Diagnostic> = diagnostics.iter().collect();
    sorted.sort_by_key(|d| (d.span.start, d.span.end));

    let mut out = String::from("{\n  \"file\": ");
    push_string(&mut out, &file.name);
    out.push_str(",\n  \"diagnostics\": [");
    for (i, d) in sorted.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n    {\n");
        out.push_str("      \"severity\": ");
        push_string(&mut out, if d.is_error() { "error" } else { "warning" });
        out.push_str(",\n      \"code\": ");
        push_string(&mut out, d.code);
        out.push_str(",\n      \"message\": ");
        push_string(&mut out, &d.message);

        let (start_line, start_col) = file.line_col(d.span.start);
        let (end_line, end_col) = file.line_col(d.span.end);
        out.push_str(&format!(
            ",\n      \"start\": {{ \"line\": {start_line}, \"column\": {start_col}, \"offset\": {} }}",
            d.span.start
        ));
        out.push_str(&format!(
            ",\n      \"end\": {{ \"line\": {end_line}, \"column\": {end_col}, \"offset\": {} }}",
            d.span.end
        ));

        out.push_str(",\n      \"label\": ");
        push_optional(&mut out, d.label.as_deref());
        out.push_str(",\n      \"help\": ");
        push_optional(&mut out, d.help.as_deref());
        out.push_str(",\n      \"notes\": [");
        for (j, note) in d.notes.iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            push_string(&mut out, note);
        }
        out.push_str("]\n    }");
    }
    if !sorted.is_empty() {
        out.push('\n');
        out.push_str("  ");
    }
    out.push_str("]\n}\n");
    out
}

fn push_optional(out: &mut String, value: Option<&str>) {
    match value {
        Some(v) => push_string(out, v),
        None => out.push_str("null"),
    }
}

/// A JSON string literal, escaping what the format requires.
fn push_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Everything below space has to be escaped; the rest can go
            // through as UTF-8.
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagnostic, Span};

    fn file() -> SourceFile {
        SourceFile::new("t.kov", "let x = 1;\nlet y = 2;\n")
    }

    #[test]
    fn an_empty_run_is_still_a_document() {
        let out = render_json(&[], &file());
        assert!(out.contains("\"diagnostics\": []"), "{out}");
        assert!(out.contains("\"file\": \"t.kov\""), "{out}");
    }

    #[test]
    fn positions_are_one_based_lines_and_columns_plus_offsets() {
        let d = Diagnostic::error("E0000", "boom", Span::new(15, 16));
        let out = render_json(&[d], &file());
        // Offset 15 is on line 2, column 5.
        assert!(
            out.contains("\"line\": 2, \"column\": 5, \"offset\": 15"),
            "{out}"
        );
    }

    #[test]
    fn absent_parts_are_null_rather_than_missing() {
        let d = Diagnostic::error("E0000", "boom", Span::new(0, 1));
        let out = render_json(&[d], &file());
        assert!(out.contains("\"label\": null"), "{out}");
        assert!(out.contains("\"help\": null"), "{out}");
        assert!(out.contains("\"notes\": []"), "{out}");
    }

    #[test]
    fn strings_are_escaped() {
        let d = Diagnostic::error(
            "E0000",
            "a \"quoted\" \\ thing\nand a newline",
            Span::new(0, 1),
        )
        .with_note("tab\there");
        let out = render_json(&[d], &file());
        assert!(
            out.contains(r#"a \"quoted\" \\ thing\nand a newline"#),
            "{out}"
        );
        assert!(out.contains(r"tab\there"), "{out}");
        // A control character becomes a \u escape.
        let d = Diagnostic::error("E0000", "bell\u{7}", Span::new(0, 1));
        assert!(render_json(&[d], &file()).contains(r"bell\u0007"));
    }

    #[test]
    fn diagnostics_come_out_in_source_order() {
        let out = render_json(
            &[
                Diagnostic::error("E0002", "second", Span::new(11, 12)),
                Diagnostic::error("E0001", "first", Span::new(0, 1)),
            ],
            &file(),
        );
        assert!(
            out.find("first").unwrap() < out.find("second").unwrap(),
            "{out}"
        );
    }
}
