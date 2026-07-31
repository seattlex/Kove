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
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("no errors found"));
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
fn new_scaffolds_a_runnable_project() {
    let parent = std::env::temp_dir().join("kove-new-test");
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_kove"))
        .args(["new", "demo_app"])
        .current_dir(&parent)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(parent.join("demo_app/kove.toml").is_file());
    assert!(parent.join("demo_app/src/main.kov").is_file());

    // `kove run` with no arguments picks up the project.
    let out = Command::new(env!("CARGO_BIN_EXE_kove"))
        .arg("run")
        .current_dir(parent.join("demo_app"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Hello from demo_app!\n"
    );
}

#[test]
fn new_rejects_bad_names_and_existing_directories() {
    let parent = std::env::temp_dir().join("kove-new-test-bad");
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(parent.join("taken")).unwrap();

    let run = |name: &str| {
        Command::new(env!("CARGO_BIN_EXE_kove"))
            .args(["new", name])
            .current_dir(&parent)
            .output()
            .unwrap()
    };
    let out = run("my-project");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("underscores"));
    assert_eq!(run("taken").status.code(), Some(2));
}

#[test]
fn broken_manifests_are_reported_with_line_numbers() {
    let dir = std::env::temp_dir().join("kove-manifest-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("kove.toml"), "[package]\nname = my_app\n").unwrap();
    std::fs::write(dir.join("src/main.kov"), "fn main() { }").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_kove"))
        .arg("check")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("kove.toml: line 2"), "{stderr}");
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

#[test]
fn fmt_rewrites_a_file_and_check_reports_without_writing() {
    let dir = std::env::temp_dir().join("kove-fmt-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("messy.kov");
    let messy = "fn  main( ){let x=1;println( x );}\n";
    std::fs::write(&file, messy).unwrap();

    // --check reports and exits 1, leaving the file alone.
    let out = kove(&["fmt", "--check", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("would reformat"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), messy);

    // Without --check it rewrites.
    let out = kove(&["fmt", file.to_str().unwrap()]);
    assert!(out.status.success());
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "fn main() {\n    let x = 1;\n    println(x);\n}\n"
    );

    // And now there is nothing to do.
    let out = kove(&["fmt", "--check", file.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn fmt_refuses_a_file_that_does_not_parse() {
    let dir = std::env::temp_dir().join("kove-fmt-broken");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("broken.kov");
    let broken = "fn main() { let x = 1 }\n";
    std::fs::write(&file, broken).unwrap();

    let out = kove(&["fmt", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("does not parse"), "{stderr}");
    // The file is untouched.
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
}

#[test]
fn fmt_walks_directories() {
    let dir = std::env::temp_dir().join("kove-fmt-dir/src/nested");
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("kove-fmt-dir"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.kov"), "fn a(){}\n").unwrap();
    std::fs::write(dir.parent().unwrap().join("b.kov"), "fn b(){}\n").unwrap();

    let root = std::env::temp_dir().join("kove-fmt-dir");
    let out = kove(&["fmt", root.to_str().unwrap()]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("2 of 2 file(s) changed"), "{text}");
}

#[test]
fn fmt_rejects_unknown_options() {
    let out = kove(&["fmt", "--wat"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn test_runs_test_functions_and_reports_failures() {
    let dir = std::env::temp_dir().join("kove-test-cmd");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("suite.kov");
    std::fs::write(
        &file,
        "fn double(n: Int) -> Int { return n * 2; }\n\
         fn test_passes() { assert(double(2) == 4); }\n\
         fn test_fails() { assert(double(3) == 7); }\n\
         fn main() { }\n",
    )
    .unwrap();

    let out = kove(&["test", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("ok    test_passes"), "{stdout}");
    assert!(stdout.contains("FAIL  test_fails"), "{stdout}");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error[E0306]"), "{stderr}");
    assert!(stderr.contains("1 passed, 1 failed"), "{stderr}");
}

#[test]
fn test_succeeds_when_every_test_passes() {
    let dir = std::env::temp_dir().join("kove-test-pass");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("suite.kov");
    std::fs::write(
        &file,
        "fn test_a() { assert(1 == 1); }\nfn test_b() { assert(true); }\nfn main() { }\n",
    )
    .unwrap();

    let out = kove(&["test", file.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("2 passed"));
}

#[test]
fn test_says_so_when_there_are_no_tests() {
    let out = kove(&["test", &example("hello.kov")]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("no tests found"));
}

#[test]
fn a_test_function_with_the_wrong_signature_is_reported() {
    let dir = std::env::temp_dir().join("kove-test-sig");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("suite.kov");
    std::fs::write(&file, "fn test_wrong(a: Int) { }\nfn main() { }\n").unwrap();

    let out = kove(&["test", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error[E0220]"), "{stderr}");
}

#[test]
fn tests_do_not_need_a_main() {
    let dir = std::env::temp_dir().join("kove-test-nomain");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("lib.kov");
    std::fs::write(&file, "fn test_a() { assert(true); }\n").unwrap();

    let out = kove(&["test", file.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn explain_prints_a_code_and_rejects_an_unknown_one() {
    let out = kove(&["explain", "E0012"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("E0012: mismatched types"), "{text}");
    assert!(
        text.contains("never converts between types implicitly"),
        "{text}"
    );

    // Case does not matter.
    assert!(kove(&["explain", "e0012"]).status.success());
    assert!(kove(&["explain", "w0001"]).status.success());

    let out = kove(&["explain", "E9999"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("not a diagnostic code"));

    // And it needs an argument.
    assert_eq!(kove(&["explain"]).status.code(), Some(2));
}

#[test]
fn explain_list_shows_every_code() {
    let out = kove(&["explain", "--list"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("E0012  mismatched types"), "{text}");
    assert!(text.contains("W0001"), "{text}");
    assert!(
        text.contains("errors:") && text.contains("warnings:"),
        "{text}"
    );
    // The count in the footer matches what the registry holds.
    assert!(
        text.contains(&format!("{} codes.", kove_diagnostics::CODES.len())),
        "{text}"
    );
}
