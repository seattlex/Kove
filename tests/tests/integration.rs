//! End-to-end fixture tests over `tests/programs/`:
//!
//! - `valid/*.kov` — must compile cleanly and produce the output recorded
//!   in the sibling `*.stdout` file.
//! - `invalid/*.kov` — must fail to compile with every code named in
//!   `// expect: E0012` comments.
//! - `runtime/*.kov` — must compile cleanly and stop with the runtime
//!   error named in a `// expect-runtime: E0301` comment.

use kove_tests::programs_dir;
use std::fs;
use std::path::Path;

fn kov_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "kov"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no fixtures in {}", dir.display());
    files
}

fn expected_codes(text: &str, marker: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix(marker))
        .map(|c| c.trim().to_string())
        .collect()
}

#[test]
fn valid_programs_run_and_match_expected_output() {
    for path in kov_files(&programs_dir().join("valid")) {
        let text = fs::read_to_string(&path).unwrap();
        let expected = fs::read_to_string(path.with_extension("stdout"))
            .unwrap_or_else(|_| panic!("missing .stdout for {}", path.display()));
        let out = kove_tests::run(&text);
        assert_eq!(out, expected, "output mismatch for {}", path.display());
    }
}

#[test]
fn invalid_programs_fail_with_the_documented_codes() {
    for path in kov_files(&programs_dir().join("invalid")) {
        let text = fs::read_to_string(&path).unwrap();
        let expected = expected_codes(&text, "// expect:");
        assert!(
            !expected.is_empty(),
            "{} must declare `// expect: E....` lines",
            path.display()
        );
        let found = kove_tests::codes(&text);
        for code in &expected {
            assert!(
                found.iter().any(|c| c == code),
                "{}: expected {code}, got {found:?}",
                path.display()
            );
        }
    }
}

#[test]
fn runtime_fixtures_stop_with_the_documented_error() {
    for path in kov_files(&programs_dir().join("runtime")) {
        let text = fs::read_to_string(&path).unwrap();
        let expected = expected_codes(&text, "// expect-runtime:");
        assert_eq!(
            expected.len(),
            1,
            "{} must declare exactly one `// expect-runtime:` line",
            path.display()
        );
        let code = kove_tests::run_expecting_runtime_error(&text);
        assert_eq!(code, expected[0], "for {}", path.display());
    }
}
