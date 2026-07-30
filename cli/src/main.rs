//! The `kove` command-line interface.
//!
//! Exit codes (stable, for scripts and CI):
//!   0 - success
//!   1 - the program has compile-time or runtime errors
//!   2 - the CLI itself was used incorrectly / the feature is unavailable

use kove_cli::project;
use kove_cli::Compilation;
use kove_diagnostics::render_all;
use std::process::ExitCode;

const USAGE: &str = "\
kove - the Kove language toolchain

USAGE:
    kove <command> [arguments]

COMMANDS:
    new <name>      Create a new project (kove.toml + src/main.kov)
    build [file]    Check the program the way `run` would (no native backend yet)
    run [file]      Compile and execute the program
    check [file]    Report diagnostics without running
    fmt             Format source files (not implemented yet)
    version         Print the toolchain version
    help            Print this message

When [file] is omitted, kove looks for a project (kove.toml with
src/main.kov), then for a plain src/main.kov or main.kov.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    match command {
        Some("new") => new_project(args.get(1)),
        Some("build") => build_or_check(args.get(1), Mode::Build),
        Some("run") => run(args.get(1)),
        Some("check") => build_or_check(args.get(1), Mode::Check),
        Some("fmt") => {
            eprintln!("`kove fmt` is not implemented yet; the formatter is a later roadmap phase.");
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

fn new_project(name: Option<&String>) -> ExitCode {
    let Some(name) = name else {
        eprintln!("error: `kove new` needs a project name, like `kove new my_project`");
        return ExitCode::from(2);
    };
    match project::scaffold(name, std::path::Path::new(".")) {
        Ok(files) => {
            println!("created `{name}`");
            for f in files {
                println!("  {}", f.display());
            }
            println!("\nnext: cd {name} && kove run");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Resolve the file to operate on: an explicit argument, else the current
/// project's entry point, else the bare-file fallbacks.
fn resolve_file(arg: Option<&String>) -> Result<String, ExitCode> {
    if let Some(path) = arg {
        if std::path::Path::new(path).is_file() {
            return Ok(path.clone());
        }
        eprintln!("error: file not found: `{path}`");
        return Err(ExitCode::from(2));
    }
    if std::path::Path::new("kove.toml").is_file() {
        // Inside a project the manifest must parse, and the entry point is
        // always src/main.kov.
        let text = std::fs::read_to_string("kove.toml").map_err(|err| {
            eprintln!("error: could not read `kove.toml`: {err}");
            ExitCode::from(2)
        })?;
        if let Err(err) = project::Manifest::parse(&text) {
            eprintln!("error: kove.toml: {err}");
            return Err(ExitCode::from(2));
        }
        if std::path::Path::new("src/main.kov").is_file() {
            return Ok("src/main.kov".to_string());
        }
        eprintln!("error: this project has no `src/main.kov`");
        return Err(ExitCode::from(2));
    }
    for candidate in ["src/main.kov", "main.kov"] {
        if std::path::Path::new(candidate).is_file() {
            return Ok(candidate.to_string());
        }
    }
    eprintln!(
        "error: no input file. Pass a path, run inside a project, or create one with `kove new`."
    );
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

/// Print diagnostics and the closing summary line. Returns the failure
/// exit code when there were errors.
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
