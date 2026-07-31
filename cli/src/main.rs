//! The `kove` command-line interface.
//!
//! Exit codes (stable, for scripts and CI):
//!   0 - success
//!   1 - the program has compile-time or runtime errors
//!   2 - the CLI itself was used incorrectly / the feature is unavailable

use kove_cli::Compilation;
use kove_diagnostics::render_all;
use kove_manifest as project;
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
    test [file]     Run every `test_...` function in the program
    explain <code>  Explain a diagnostic code, such as E0012
                    (--list to show every code)
    fmt [path]...   Format .kov files in place (--check to only report)
    version         Print the toolchain version
    help            Print this message

When [file] is omitted, kove looks for a project (kove.toml with
src/main.kov), then for a plain src/main.kov or main.kov. `kove fmt`
with no path formats the whole project, or the current directory.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    match command {
        Some("new") => new_project(args.get(1)),
        Some("build") => build_or_check(args.get(1), Mode::Build),
        Some("run") => run(args.get(1)),
        Some("check") => build_or_check(args.get(1), Mode::Check),
        Some("test") => test(args.get(1)),
        Some("explain") => explain(args.get(1)),
        Some("fmt") => fmt(&args[1..]),
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
/// exit code when there were errors; warnings are printed but do not stop
/// anything.
fn report(c: &Compilation) -> Option<ExitCode> {
    if c.diagnostics.is_empty() {
        return None;
    }
    eprintln!("{}", render_all(&c.diagnostics, &c.source));

    let errors = c.error_count();
    let warnings = c.warning_count();
    if errors == 0 {
        eprintln!(
            "warning: `{}` generated {} warning{}",
            c.source.name,
            warnings,
            plural(warnings)
        );
        return None;
    }
    if warnings > 0 {
        eprintln!(
            "warning: `{}` generated {} warning{}",
            c.source.name,
            warnings,
            plural(warnings)
        );
    }
    eprintln!(
        "error: could not compile `{}` due to {} previous error{}",
        c.source.name,
        errors,
        plural(errors)
    );
    Some(ExitCode::from(1))
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
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
             note: native code generation is not implemented yet (v0.6); \
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

/// `kove fmt [--check] [path]...`
///
/// Rewrites `.kov` files in place. With `--check` nothing is written and
/// the exit code says whether anything would change, which is what CI
/// wants.
fn fmt(args: &[String]) -> ExitCode {
    let check_only = args.iter().any(|a| a == "--check");
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    for arg in args.iter().filter(|a| a.starts_with("--")) {
        if arg != "--check" {
            eprintln!("error: unknown option `{arg}` for `kove fmt`");
            return ExitCode::from(2);
        }
    }

    let mut files = Vec::new();
    if paths.is_empty() {
        // No path: the project's src/, or the current directory.
        let root = if std::path::Path::new("src").is_dir() {
            std::path::PathBuf::from("src")
        } else {
            std::path::PathBuf::from(".")
        };
        collect_kov_files(&root, &mut files);
    } else {
        for p in paths {
            let path = std::path::PathBuf::from(p);
            if path.is_dir() {
                collect_kov_files(&path, &mut files);
            } else if path.is_file() {
                files.push(path);
            } else {
                eprintln!("error: no such file or directory: `{p}`");
                return ExitCode::from(2);
            }
        }
    }

    if files.is_empty() {
        eprintln!("error: no `.kov` files found");
        return ExitCode::from(2);
    }
    files.sort();

    let mut changed = Vec::new();
    let mut failed = false;
    for path in &files {
        let display = path.display().to_string();
        let Ok(source) = std::fs::read_to_string(path) else {
            eprintln!("error: could not read `{display}`");
            failed = true;
            continue;
        };
        match kove_formatter::format(&source) {
            Ok(formatted) => {
                if formatted == source {
                    continue;
                }
                changed.push(display.clone());
                if !check_only {
                    if let Err(err) = std::fs::write(path, formatted) {
                        eprintln!("error: could not write `{display}`: {err}");
                        failed = true;
                    }
                }
            }
            Err(diags) => {
                // Formatting a file the compiler rejects would be guessing.
                let file = kove_diagnostics::SourceFile::new(display.clone(), source);
                eprintln!("{}", render_all(&diags, &file));
                eprintln!("error: cannot format `{display}` because it does not parse");
                failed = true;
            }
        }
    }

    if failed {
        return ExitCode::from(1);
    }
    if check_only {
        if changed.is_empty() {
            println!("all {} file(s) are formatted", files.len());
            return ExitCode::SUCCESS;
        }
        for c in &changed {
            println!("would reformat {c}");
        }
        return ExitCode::from(1);
    }
    match changed.len() {
        0 => println!("all {} file(s) already formatted", files.len()),
        n => {
            for c in &changed {
                println!("formatted {c}");
            }
            println!("{n} of {} file(s) changed", files.len());
        }
    }
    ExitCode::SUCCESS
}

fn collect_kov_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Build output is not source.
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_kov_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "kov") {
            out.push(path);
        }
    }
}

/// `kove test [file]`
///
/// Compiles the program and runs every `test_` function in declaration
/// order. A test passes if it finishes; it fails if it hits a runtime
/// error, which is what `assert` produces.
fn test(arg: Option<&String>) -> ExitCode {
    let (path, text) = match load(arg) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let c = kove_cli::compile_tests(&path, &text);
    if let Some(code) = report(&c) {
        return code;
    }

    let tests = kove_cli::test_functions(&c.program);
    if tests.is_empty() {
        println!("no tests found in `{path}`");
        println!("note: a test is a function named `test_...` taking no parameters");
        return ExitCode::SUCCESS;
    }

    println!("running {} test(s) in `{}`", tests.len(), path);
    let mut failures = Vec::new();
    for f in &tests {
        let name = f.name.name.clone();
        match kove_cli::run_test(&c, &name) {
            Ok(output) => {
                println!("  ok    {name}");
                // A passing test's output is not interesting, but a
                // passing test that printed something might be.
                if !output.is_empty() {
                    for line in String::from_utf8_lossy(&output).lines() {
                        println!("          {line}");
                    }
                }
            }
            Err(diag) => {
                println!("  FAIL  {name}");
                failures.push((name, diag));
            }
        }
    }

    let passed = tests.len() - failures.len();
    if failures.is_empty() {
        println!("\n{passed} passed");
        return ExitCode::SUCCESS;
    }
    for (name, diag) in &failures {
        eprintln!("\n---- {name} ----");
        eprintln!("{}", kove_diagnostics::render(diag, &c.source));
    }
    eprintln!("{passed} passed, {} failed", failures.len());
    ExitCode::from(1)
}

/// `kove explain <code>`
///
/// Prints the long form of a diagnostic code. The codes are stable, so
/// this is also what documentation and search results can point at.
fn explain(code: Option<&String>) -> ExitCode {
    let Some(code) = code else {
        eprintln!("error: `kove explain` needs a diagnostic code, like `kove explain E0012`");
        eprintln!("note: `kove explain --list` shows every code");
        return ExitCode::from(2);
    };
    if code == "--list" {
        return list_codes();
    }
    match kove_diagnostics::explain(code) {
        Some(info) => {
            println!("{}: {}\n", info.code, info.summary);
            println!("{}", info.explanation);
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("error: `{code}` is not a diagnostic code Kove emits");
            // A wrong prefix is the most likely mistake, so say what the
            // bands mean rather than just refusing.
            eprintln!(
                "note: errors are `E....` and warnings are `W....`; \
                 the full list is in docs/diagnostics.md"
            );
            ExitCode::from(2)
        }
    }
}

/// `kove explain --list`
///
/// Every code with its summary, so the set is discoverable without
/// opening the documentation.
fn list_codes() -> ExitCode {
    let width = kove_diagnostics::CODES
        .iter()
        .map(|c| c.code.len())
        .max()
        .unwrap_or(5);
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for info in kove_diagnostics::CODES {
        if info.code.starts_with('W') {
            warnings.push(info);
        } else {
            errors.push(info);
        }
    }
    errors.sort_by_key(|c| c.code);
    warnings.sort_by_key(|c| c.code);

    println!("errors:");
    for info in &errors {
        println!("  {:width$}  {}", info.code, info.summary);
    }
    println!("\nwarnings:");
    for info in &warnings {
        println!("  {:width$}  {}", info.code, info.summary);
    }
    println!(
        "\n{} codes. `kove explain <code>` for any of them.",
        errors.len() + warnings.len()
    );
    ExitCode::SUCCESS
}
