//! The compiler driver: wires the pipeline stages together.
//!
//! ```text
//! source text ── kove-lexer + kove-parser ──> syntax tree + diagnostics
//!                     │ kove-ast
//!                     ▼
//!                    AST ── kove-resolver ──> what each name refers to
//!                     │                       │
//!                     └──> kove-typechecker <─┘ ──> semantic diagnostics
//!                     │ kove-interpreter (when running)
//!                     ▼
//!                  output
//! ```
//!
//! The semantic stages only run when the syntax phase produced no
//! errors, since a broken parse would drown the user in follow-on errors.
//! Within a phase, every error found is reported. Warnings never block a
//! stage and never fail a build.

use kove_ast::Program;
use kove_diagnostics::{Diagnostic, SourceFile};
use std::io::Write;

pub struct Compilation {
    pub source: SourceFile,
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

impl Compilation {
    /// True if anything blocks compilation. Warnings do not.
    pub fn has_errors(&self) -> bool {
        kove_diagnostics::has_errors(&self.diagnostics)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_warning()).count()
    }
}

/// Run the compiler frontend (parse, lower, type-check) over one file.
/// `name` is what diagnostics display as the file path.
pub fn compile(name: &str, text: &str) -> Compilation {
    let doc = kove_parser::parse(text);
    let mut diagnostics = kove_parser::syntax_diagnostics(&doc);
    let lowered = kove_ast::lower(&doc);
    diagnostics.extend(lowered.diagnostics);
    // Warnings do not stop the later stages; only errors do, because
    // semantic errors on top of a broken parse are noise.
    if !kove_diagnostics::has_errors(&diagnostics) {
        // Names first, then types. The type checker consumes the
        // resolver's output and never looks a name up itself.
        let (resolutions, resolve_diags) = kove_resolver::resolve(&lowered.program);
        diagnostics.extend(resolve_diags);
        diagnostics.extend(kove_typechecker::check(&lowered.program, &resolutions));
    }
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end));
    Compilation {
        source: SourceFile::new(name, text),
        program: lowered.program,
        diagnostics,
    }
}

/// Frontend plus the `main` checks an executable program needs.
pub fn compile_executable(name: &str, text: &str) -> Compilation {
    let mut c = compile(name, text);
    if !c.has_errors() {
        c.diagnostics
            .extend(kove_typechecker::check_main(&c.program));
    }
    c
}

/// Stack reserved for the interpreter thread. The tree-walking evaluator
/// uses host stack proportional to Kove call depth, so the driver gives it
/// a stack sized for [`kove_interpreter::RECURSION_LIMIT`] with room to
/// spare. Kove's own recursion limit must be what stops runaway recursion,
/// never the host stack. (Reserved virtual memory; only pages actually
/// used are committed.)
const INTERPRETER_STACK: usize = 256 * 1024 * 1024;

/// The test functions in a program: every function whose name starts
/// with `test_`, in declaration order.
///
/// Kove has no attributes, so a naming convention is what marks a test.
/// Only functions that take no parameters and return nothing qualify;
/// `check_tests` reports the ones that look like tests but cannot be run.
pub fn test_functions(program: &Program) -> Vec<&kove_ast::Function> {
    program
        .items
        .iter()
        .filter_map(|i| match i {
            kove_ast::Item::Function(f)
                if f.name.name.starts_with("test_")
                    && f.params.is_empty()
                    && f.return_type.is_none() =>
            {
                Some(f)
            }
            _ => None,
        })
        .collect()
}

/// Frontend plus the checks `kove test` needs. A `test_` function that
/// takes parameters or returns a value cannot be run by the harness, and
/// silently skipping it would be worse than saying so.
pub fn compile_tests(name: &str, text: &str) -> Compilation {
    let mut c = compile(name, text);
    if c.has_errors() {
        return c;
    }
    for item in &c.program.items {
        let kove_ast::Item::Function(f) = item else {
            continue;
        };
        if !f.name.name.starts_with("test_") {
            continue;
        }
        if !f.params.is_empty() || f.return_type.is_some() {
            c.diagnostics.push(
                Diagnostic::error(
                    "E0220",
                    format!(
                        "`{}` looks like a test but cannot be run as one",
                        f.name.name
                    ),
                    f.name.span,
                )
                .with_label("test functions take no parameters and return nothing")
                .with_help(format!("change the signature to `fn {}()`", f.name.name)),
            );
        }
    }
    c
}

/// Run one test function, returning its output on success or the runtime
/// diagnostic on failure.
// Unboxed for the same reason as `run`: one per test, so the caller's
// convenience beats shrinking a `Result` built at most once.
#[allow(clippy::result_large_err)]
pub fn run_test(c: &Compilation, name: &str) -> Result<Vec<u8>, Diagnostic> {
    let mut out = Vec::new();
    match run_entry(c, name, &mut out) {
        Ok(()) => Ok(out),
        Err(d) => Err(d),
    }
}

/// Execute a clean compilation's `main`, writing program output to `out`.
/// On a runtime error, returns the renderable diagnostic.
// The diagnostic is returned unboxed on purpose: this runs once per
// program, so the caller's convenience beats shrinking a `Result` that is
// constructed at most once.
#[allow(clippy::result_large_err)]
pub fn run(c: &Compilation, out: &mut (dyn Write + Send)) -> Result<(), Diagnostic> {
    run_entry(c, "main", out)
}

/// Execute one named entry point on the interpreter thread.
#[allow(clippy::result_large_err)]
fn run_entry(c: &Compilation, entry: &str, out: &mut (dyn Write + Send)) -> Result<(), Diagnostic> {
    debug_assert!(!c.has_errors());
    let result = std::thread::scope(|s| {
        let handle = std::thread::Builder::new()
            .name("kove-interpreter".into())
            .stack_size(INTERPRETER_STACK)
            .spawn_scoped(s, || kove_interpreter::run_function(&c.program, entry, out))
            .expect("failed to spawn the interpreter thread");
        match handle.join() {
            Ok(result) => result,
            // An interpreter panic is an internal compiler error; keep its
            // message instead of masking it.
            Err(panic) => std::panic::resume_unwind(panic),
        }
    });
    result.map_err(kove_interpreter::RuntimeError::into_diagnostic)
}
