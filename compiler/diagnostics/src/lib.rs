//! Diagnostic types shared by every stage of the Kove compiler, plus the
//! terminal renderer that produces the `error[E0012]: ...` output format.
//!
//! Every error carries a stable code (`E0012`), a byte span into the source,
//! and optionally a label (printed under the carets), a help suggestion and
//! free-form notes. The full code registry lives in `docs/diagnostics.md`.

use std::fmt;

/// A byte range into a source file. Produced by the frontend and preserved
/// through every compiler stage so diagnostics can always point back at
/// source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Span {
        Span { start, end }
    }

    pub fn empty(at: u32) -> Span {
        Span { start: at, end: at }
    }

    /// The smallest span covering both.
    pub fn cover(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One diagnostic. Construct with [`Diagnostic::error`] and chain the
/// `with_*` methods for the optional parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    /// Short text printed right after the `^^^` markers.
    pub label: Option<String>,
    /// A `help: ...` suggestion printed after the snippet.
    pub help: Option<String>,
    /// `note: ...` lines printed after the snippet.
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            span,
            label: None,
            help: None,
            notes: Vec::new(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(code, message, span)
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Diagnostic {
        self.label = Some(label.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Diagnostic {
        self.help = Some(help.into());
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Diagnostic {
        self.notes.push(note.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn is_warning(&self) -> bool {
        self.severity == Severity::Warning
    }
}

/// True if any diagnostic is an error. Warnings never stop a build, and
/// never stop a later compiler stage from running either.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(Diagnostic::is_error)
}

/// A named source file with a precomputed line index.
pub struct SourceFile {
    pub name: String,
    pub text: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> SourceFile {
        let text = text.into();
        let mut line_starts = vec![0u32];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i as u32 + 1);
            }
        }
        SourceFile {
            name: name.into(),
            text,
            line_starts,
        }
    }

    /// Zero-based line index containing the byte offset.
    fn line_of(&self, offset: u32) -> usize {
        self.line_starts.partition_point(|&s| s <= offset) - 1
    }

    /// One-based `(line, column)` of a byte offset. Columns count characters.
    pub fn line_col(&self, offset: u32) -> (usize, usize) {
        let offset = offset.min(self.text.len() as u32);
        let line = self.line_of(offset);
        let start = self.line_starts[line] as usize;
        let col = self.text[start..offset as usize].chars().count() + 1;
        (line + 1, col)
    }

    fn line_start(&self, line: usize) -> u32 {
        self.line_starts[line]
    }

    /// The text of a zero-based line, without the trailing newline.
    fn line_text(&self, line: usize) -> &str {
        let start = self.line_starts[line] as usize;
        let end = self
            .line_starts
            .get(line + 1)
            .map(|&s| s as usize)
            .unwrap_or(self.text.len());
        self.text[start..end].trim_end_matches(['\n', '\r'])
    }
}

/// Render one diagnostic in the format documented in `docs/diagnostics.md`:
///
/// ```text
/// error[E0012]: mismatched types: expected `Int`, found `String`
///  --> src/main.kov:4:20
///   |
/// 4 |     let age: Int = "sixteen";
///   |                    ^^^^^^^^^ expected `Int`
///   |
/// help: remove the quotes or change the variable type
/// ```
pub fn render(diag: &Diagnostic, file: &SourceFile) -> String {
    let (line_no, col) = file.line_col(diag.span.start);
    let line_idx = line_no - 1;
    let raw_line = file.line_text(line_idx);
    let line_start = file.line_start(line_idx);

    // Tabs render as four spaces; the caret columns account for that.
    let span_start_in_line = (diag.span.start.saturating_sub(line_start)) as usize;
    let span_end_in_line = (diag.span.end.max(diag.span.start) - line_start) as usize;
    let mut rendered = String::new();
    let mut prefix_cols = 0usize;
    let mut caret_cols = 0usize;
    for (i, ch) in raw_line.char_indices() {
        let w = if ch == '\t' { 4 } else { 1 };
        if ch == '\t' {
            rendered.push_str("    ");
        } else {
            rendered.push(ch);
        }
        if i < span_start_in_line {
            prefix_cols += w;
        } else if i < span_end_in_line {
            caret_cols += w;
        }
    }
    if span_start_in_line >= raw_line.len() {
        prefix_cols = rendered.chars().count();
    }
    let caret_cols = caret_cols.max(1);

    let num = line_no.to_string();
    let gut = num.len();
    let bar = format!("{} |", " ".repeat(gut));

    let mut out = String::new();
    out.push_str(&format!(
        "{}[{}]: {}\n",
        diag.severity.as_str(),
        diag.code,
        diag.message
    ));
    out.push_str(&format!(
        "{}--> {}:{}:{}\n",
        " ".repeat(gut),
        file.name,
        line_no,
        col
    ));
    out.push_str(&bar);
    out.push('\n');
    out.push_str(&format!("{} | {}\n", num, rendered));
    let marker = if diag.severity == Severity::Error {
        "^"
    } else {
        "-"
    };
    let mut underline = format!(
        "{} {}{}",
        bar,
        " ".repeat(prefix_cols),
        marker.repeat(caret_cols)
    );
    if let Some(label) = &diag.label {
        underline.push(' ');
        underline.push_str(label);
    }
    out.push_str(underline.trim_end());
    out.push('\n');
    if diag.help.is_some() || !diag.notes.is_empty() {
        out.push_str(&bar);
        out.push('\n');
    }
    if let Some(help) = &diag.help {
        out.push_str(&format!("help: {}\n", help));
    }
    for note in &diag.notes {
        out.push_str(&format!("note: {}\n", note));
    }
    out
}

/// Render a batch of diagnostics separated by blank lines, sorted by
/// position (stable for equal positions, so stage order is preserved).
pub fn render_all(diags: &[Diagnostic], file: &SourceFile) -> String {
    let mut sorted: Vec<&Diagnostic> = diags.iter().collect();
    sorted.sort_by_key(|d| (d.span.start, d.span.end));
    let mut out = String::new();
    for (i, d) in sorted.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&render(d, file));
    }
    out
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
