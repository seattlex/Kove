//! The Kove formatter.
//!
//! Opinionated and deterministic: no options, no configuration file. The
//! same input always produces the same output, and formatting formatted
//! code changes nothing.
//!
//! It works from the concrete syntax tree rather than the AST, because the
//! AST has thrown away exactly what a formatter needs: comments, and the
//! distinction between what the user wrote and what it meant. ReParse
//! trees keep every byte, so nothing can be silently dropped.
//!
//! What the formatter decides:
//!
//! - four-space indentation, one statement per line
//! - one space around binary operators, none around `..`, none inside
//!   parentheses
//! - one field per line in struct and enum declarations
//! - a single blank line where the author left one or more; never two
//!
//! What it deliberately leaves alone:
//!
//! - redundant parentheses. Removing them requires reasoning about
//!   precedence, and a formatter that can change how an expression groups
//!   is a formatter nobody should trust.
//! - the author's choice of single-line or multi-line for constructs that
//!   fit either way is *not* preserved; width decides, so the output is a
//!   function of the code alone.

mod printer;

use kove_diagnostics::Diagnostic;
use printer::Printer;

/// The column the formatter tries to stay within.
pub const MAX_WIDTH: usize = 100;

/// Format Kove source.
///
/// Returns the syntax diagnostics unchanged if the input does not parse
/// cleanly. The tree would still be complete enough to walk, but rewriting
/// a file whose syntax the compiler rejects is not something a formatter
/// should do behind the user's back.
pub fn format(source: &str) -> Result<String, Vec<Diagnostic>> {
    let doc = kove_parser::parse(source);
    let diagnostics = kove_parser::syntax_diagnostics(&doc);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let lang = doc.language().clone();
    let mut printer = Printer::new(&lang, doc.text());
    printer.file(&doc.tree().root());
    Ok(printer.finish())
}
