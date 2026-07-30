//! Process-level tests of the `kove` binary: commands, exit codes, and
//! the stdout/stderr split.

use std::path::Path;
use std::process::{Command, Output};

fn kove(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kove"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run the kove binary")
}

fn example(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join(name);
    path.to_str().unwrap().to_string()
}

#[test]
fn version_prints_and_succeeds() {
    let out = kove(&["version"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("kove "), "{text}");
}

#[test]
fn run_executes_the_hello_example() {
    let out = kove(&["run", &example("hello.kov")]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "Hello, Kove!\n");
}

#[test]
fn run_executes_the_milestone_example() {
    let out = kove(&["run", &example("add.kov")]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Greater than twenty\n"
    );
}

#[test]
fn check_reports_diagnostics_on_stderr_and_exits_1() {
    let dir = std::env::temp_dir().join("kove-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.kov");
    std::fs::write(&file, "fn main() { let age: Int = \"sixteen\"; }").unwrap();

    let out = kove(&["check", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error[E0012]"), "{stderr}");
    assert!(stderr.contains("^^^^^^^^^"), "{stderr}");
    assert!(String::from_utf8(out.stdout).unwrap().is_empty());
}

#[test]
fn check_succeeds_on_a_valid_file() {
    let out = kove(&["check", &example("structs.kov")]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("no errors found"));
}

#[test]
fn build_checks_and_explains_the_missing_backend() {
    let out = kove(&["build", &example("add.kov")]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("no errors found"), "{text}");
    assert!(text.contains("native code generation"), "{text}");
}

#[test]
fn missing_file_is_a_usage_error() {
    let out = kove(&["run", "does-not-exist.kov"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_command_is_a_usage_error() {
    let out = kove(&["frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn runtime_errors_exit_1_with_a_rendered_diagnostic() {
    let dir = std::env::temp_dir().join("kove-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("crash.kov");
    std::fs::write(&file, "fn main() { let z = 0; println(1 / z); }").unwrap();

    let out = kove(&["run", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error[E0301]"), "{stderr}");
    assert!(stderr.contains("divide by zero"), "{stderr}");
}
