//! Project files: parsing `kove.toml` and scaffolding for `kove new`.
//!
//! The manifest parser is deliberately small and strict. It reads exactly
//! the subset of TOML that a Kove project uses today (a `[package]` section
//! with quoted string values, plus an empty `[dependencies]` section) and
//! rejects everything else with a message that says what to fix. When the
//! package manager lands this grows with it.

use std::path::{Path, PathBuf};

/// A parsed `kove.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
}

impl Manifest {
    /// Parse a manifest. Errors are plain strings ready to print after
    /// a `kove.toml: ` prefix.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let mut section = String::new();
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;

        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix('[') {
                let Some(header) = rest.strip_suffix(']') else {
                    return Err(format!("line {n}: section header is missing its `]`"));
                };
                section = header.trim().to_string();
                if section != "package" && section != "dependencies" {
                    return Err(format!(
                        "line {n}: unknown section `[{section}]`; kove.toml has `[package]` and `[dependencies]`"
                    ));
                }
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!(
                    "line {n}: expected `key = \"value\"`, found `{line}`"
                ));
            };
            let key = key.trim();
            match section.as_str() {
                "package" => {
                    let value = unquote(value.trim()).ok_or_else(|| {
                        format!("line {n}: the value of `{key}` must be a quoted string")
                    })?;
                    match key {
                        "name" => name = Some(value),
                        "version" => version = Some(value),
                        other => {
                            return Err(format!(
                                "line {n}: unknown key `{other}` in [package]; supported keys are `name` and `version`"
                            ))
                        }
                    }
                }
                "dependencies" => {
                    return Err(format!(
                        "line {n}: dependencies are not supported yet; the package manager is a later roadmap phase"
                    ));
                }
                _ => {
                    return Err(format!(
                        "line {n}: `{key}` is outside of any section; start the file with `[package]`"
                    ));
                }
            }
        }

        let name = name.ok_or("missing `name` in [package]")?;
        if !valid_name(&name) {
            return Err(format!("`{name}` is not a valid package name; {NAME_RULE}"));
        }
        let version = version.ok_or("missing `version` in [package]")?;
        Ok(Manifest { name, version })
    }
}

const NAME_RULE: &str =
    "use letters, digits and underscores, starting with a letter (package names double as future module names, so they follow identifier rules)";

/// Package names follow identifier rules because a package will one day be
/// importable as a module.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A quoted string value with nothing but an optional `# comment` after it.
/// No escapes; manifests do not need them yet.
fn unquote(value: &str) -> Option<String> {
    let rest = value.strip_prefix('"')?;
    let end = rest.find('"')?;
    let after = rest[end + 1..].trim();
    if !(after.is_empty() || after.starts_with('#')) {
        return None;
    }
    Some(rest[..end].to_string())
}

/// Create a new project directory under `at`. Returns the files written,
/// relative to `at`, for the CLI to print.
pub fn scaffold(name: &str, at: &Path) -> Result<Vec<PathBuf>, String> {
    if !valid_name(name) {
        return Err(format!("`{name}` is not a valid project name; {NAME_RULE}"));
    }
    let root = at.join(name);
    if root.exists() {
        return Err(format!("`{}` already exists", root.display()));
    }
    let src = root.join("src");
    std::fs::create_dir_all(&src)
        .map_err(|e| format!("could not create `{}`: {e}", src.display()))?;

    let manifest_path = root.join("kove.toml");
    let manifest = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[dependencies]\n");
    std::fs::write(&manifest_path, manifest)
        .map_err(|e| format!("could not write `{}`: {e}", manifest_path.display()))?;

    let main_path = src.join("main.kov");
    let main = format!("fn main() {{\n    println(\"Hello from {name}!\");\n}}\n");
    std::fs::write(&main_path, main)
        .map_err(|e| format!("could not write `{}`: {e}", main_path.display()))?;

    Ok(vec![
        PathBuf::from(name).join("kove.toml"),
        PathBuf::from(name).join("src").join("main.kov"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_generated_manifest() {
        let m = Manifest::parse(
            "[package]\nname = \"my_project\"\nversion = \"0.1.0\"\n\n[dependencies]\n",
        )
        .unwrap();
        assert_eq!(m.name, "my_project");
        assert_eq!(m.version, "0.1.0");
    }

    #[test]
    fn comments_and_blank_lines_are_fine() {
        let m = Manifest::parse(
            "# a project\n[package]\nname = \"p\" # inline\n\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        assert_eq!(m.name, "p");
    }

    #[test]
    fn missing_fields_are_reported() {
        assert!(Manifest::parse("[package]\nversion = \"1\"\n")
            .unwrap_err()
            .contains("missing `name`"));
        assert!(Manifest::parse("[package]\nname = \"p\"\n")
            .unwrap_err()
            .contains("missing `version`"));
    }

    #[test]
    fn bad_input_gets_line_numbers() {
        assert!(Manifest::parse("[package]\nname\n")
            .unwrap_err()
            .starts_with("line 2"));
        assert!(Manifest::parse("name = \"p\"\n")
            .unwrap_err()
            .contains("outside of any section"));
        assert!(Manifest::parse("[wrong]\n")
            .unwrap_err()
            .contains("unknown section"));
        assert!(Manifest::parse("[package]\nname = 3\n")
            .unwrap_err()
            .contains("quoted string"));
        assert!(Manifest::parse("[package]\nauthor = \"x\"\n")
            .unwrap_err()
            .contains("unknown key"));
    }

    #[test]
    fn dependencies_are_rejected_for_now() {
        let err = Manifest::parse(
            "[package]\nname = \"p\"\nversion = \"1\"\n[dependencies]\nfoo = \"1\"\n",
        )
        .unwrap_err();
        assert!(err.contains("not supported yet"), "{err}");
    }

    #[test]
    fn names_follow_identifier_rules() {
        assert!(valid_name("my_project"));
        assert!(valid_name("web2"));
        assert!(!valid_name("my-project"));
        assert!(!valid_name("2fast"));
        assert!(!valid_name(""));
        assert!(!valid_name("_hidden"));
    }
}
