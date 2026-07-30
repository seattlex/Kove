//! The `kove` command-line interface.
//!
//! Exit codes (stable, for scripts and CI):
//!   0 — success
//!   1 — the program has compile-time or runtime errors
//!   2 — the CLI itself was used incorrectly / the feature is unavailable

use kove_cli::Compilation;
use kove_diagnostics::render_all;
use std::process::ExitCode;

const USAGE: &str = "\
kove — the Kove language toolchain

USAGE:
    kove <command> [file.kov]

COMMANDS:
    build [file]    Check the program the way `run` would (no native backend yet)
    run [file]      Compile and execute the program
    check [file]    Report diagnostics without running
    fmt             Format source files (not implemented yet)
    version         Print the toolchain version
    help            Print this message

When [file] is omitted, kove tries `src/main.kov`, then `main.kov`.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    match command {
        Some("build") => build_or_check(args.get(1), Mode::Build),
        Some("run") => run(args.get(1)),
        Some("check") => build_or_check(args.get(1), Mode::Check),
        Some("fmt") => {
            eprintln!("`kove fmt` is not implemented yet — the formatter is a later roadmap phase.");
            ExitCode::from(2)
        }
        Some("version") | Some("--version") | Some("-V") => {
            println!("kove {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("help") | Some("--help") | Some("-h") | None => {
            print!("{USAGE}");
            if command.is_none() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            }
        }
        Some(other) => {
            eprintln!("error: unknown command `{other}`\n");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

enum Mode {
    Build,
    Check,
}

/// Resolve the file to operate on: an explicit argument, else the standard
/// project entry points.
fn resolve_file(arg: Option<&String>) -> Result<String, ExitCode> {
    if let Some(path) = arg {
        if std::path::Path::new(path).is_file() {
            return Ok(path.clone());
        }
        eprintln!("error: file not found: `{path}`");
        return Err(ExitCode::from(2));
    }
    for candidate in ["src/main.kov", "main.kov"] {
        if std::path::Path::new(candidate).is_file() {
            return Ok(candidate.to_string());
        }
    }
    eprintln!("error: no input file. Pass a path, or create `src/main.kov`.");
    Err(ExitCode::from(2))
}

fn load(arg: Option<&String>) -> Result<(String, String), ExitCode> {
    let path = resolve_file(arg)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok((path, text)),
        Err(err) => {
            eprintln!("error: could not read `{path}`: {err}");
            Err(ExitCode::from(2))
        }
    }
}

/// Print diagnostics and the closing summary line. Returns failure exit
/// code when there were errors.
fn report(c: &Compilation) -> Option<ExitCode> {
    if !c.has_errors() {
        return None;
    }
    eprintln!("{}", render_all(&c.diagnostics, &c.source));
    let n = c.diagnostics.len();
    eprintln!(
        "error: could not compile `{}` due to {} previous error{}",
        c.source.name,
        n,
        if n == 1 { "" } else { "s" }
    );
    Some(ExitCode::from(1))
}

fn build_or_check(arg: Option<&String>, mode: Mode) -> ExitCode {
    let (path, text) = match load(arg) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let c = match mode {
        Mode::Build => kove_cli::compile_executable(&path, &text),
        Mode::Check => kove_cli::compile(&path, &text),
    };
    if let Some(code) = report(&c) {
        return code;
    }
    match mode {
        Mode::Check => println!("checked `{path}`: no errors found"),
        Mode::Build => println!(
            "checked `{path}`: no errors found\n\
             note: native code generation is not implemented yet (roadmap phase 7); \
             use `kove run {path}` to execute the program"
        ),
    }
    ExitCode::SUCCESS
}

fn run(arg: Option<&String>) -> ExitCode {
    let (path, text) = match load(arg) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let c = kove_cli::compile_executable(&path, &text);
    if let Some(code) = report(&c) {
        return code;
    }
    let mut out = std::io::stdout();
    match kove_cli::run(&c, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(diag) => {
            eprintln!("{}", kove_diagnostics::render(&diag, &c.source));
            eprintln!("error: `{path}` stopped because of a runtime error");
            ExitCode::from(1)
        }
    }
}
