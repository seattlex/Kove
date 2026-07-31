# Changelog

Notable changes to the Kove toolchain. The language is pre-1.0, so
anything documented here can still change; entries call out when
behavior does.

The format loosely follows [Keep a Changelog](https://keepachangelog.com).

The toolchain's version number tracks the [roadmap](docs/roadmap.md)
rather than running separately: `kove version` printing 0.5.0 means
roadmap v0.5 is complete. Two numbering schemes for one project would
only ever confuse people. Semantic versioning applies from 1.0; before
that a minor bump can change the language, and the changelog says when
it does.

## 0.5.0

The first version worth installing. A Kove program can be written,
checked with real diagnostics, formatted, tested and run, from a
terminal or from an editor.

### Language

- Variables with `let`, immutable by default, `let mut` to opt in
- Primitive types: `Int`, `Float`, `Bool`, `Char`, `String`
- Functions with mandatory parameter types and optional return types,
  declared in any order, recursion supported
- `if` / `else if` / `else`, `while`, and `for` over half-open Int
  ranges (`0..10`)
- Compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`), rewritten during
  lowering so it inherits every rule of the long form
- Structs with literals, field access and field assignment
- Enums with unit variants, `Enum::Variant` paths and equality
- `println`, `assert`, `to_float` and `to_int` as built-in functions,
  with reserved names. The conversions are the only way `Int` and
  `Float` meet, since neither converts on its own
- Escape sequences including `\u{...}` for a code point in hexadecimal
- Line and block comments
- `import` and `match` reserved, with `import` reporting E0217 so the
  single-file model is never mistaken for the module system to come

### Compiler

- Frontend built on [ReParse](https://github.com/seattlex/ReParse): one
  grammar gives lexing, parsing, error recovery and LSP-ready trees, so
  the compiler and the future language server cannot disagree
- One crate per pipeline stage: lexer (token vocabulary), parser
  (grammar and recovery), ast (lowering), resolver (symbol tables,
  scopes, name binding), typechecker (types only)
- Span-preserving AST whose nodes carry a `NodeId`, which is how the
  resolver hands its results to the type checker and how HIR will be
  built later
- Name resolution and type checking are separate stages: the type
  checker never looks a name up, and `Ty::Error` stops cascades so one
  mistake produces one diagnostic
- Roughly 30 stable diagnostic codes, rendered with a source snippet,
  caret markers, labels, help and notes
- Suggestions for mistyped names: unknown types, variables and functions
  propose the closest thing in scope when one is close enough
- Warnings as a separate outcome from errors, never failing a build.
  Two lints so far: W0001 for a binding whose value is never read, and
  W0002 for a function no execution can reach
- Tree-walking reference interpreter with checked Int arithmetic
  (division by zero, remainder by zero, overflow) and a recursion limit
  that reports a diagnostic instead of overflowing the host stack
- Value semantics throughout, the deliberate v0.1 memory model

### Editors

- A VS Code extension in `editors/vscode`: syntax highlighting,
  diagnostics as you type and on save, format on save, snippets, and
  commands for run, check, test, format and explain. No build step, and
  everything it shows comes from the `kove` binary

### Tooling

- `kove new` scaffolds a project; `kove build`, `kove run` and
  `kove check` resolve the current project when given no file
- `kove.toml` manifests, parsed strictly with line-numbered errors
- `kove test` runs every `test_...` function and reports failures with
  the failing assertion's source snippet
- `kove explain <code>` prints the long form of a diagnostic code, and
  `kove explain --list` shows them all, with tests keeping the registry
  and docs/diagnostics.md in step
- `kove fmt` formats in place and `kove fmt --check` reports without
  writing: opinionated, no options, idempotent, and never changes the
  token stream. Refuses files that do not parse. Enforced in CI over the
  repository's own Kove sources
- `kove check --json` prints diagnostics in a stable machine-readable
  shape, so editors and CI never scrape the human layout
- `kove check -` and `kove fmt -` read source from stdin, with `--name=`
  giving diagnostics a path to report, so an editor can check and format
  a buffer it has not saved
- Stable exit codes: 0 success, 1 program errors, 2 CLI misuse
- `cargo install --path cli` puts `kove` on your PATH

### Known gaps

No intermediate representation or native backend yet (v0.6), so
`kove build` stops after checking and `kove run` interprets. No standard
library, no modules, no pattern matching, no generics or enum payloads,
no references or ownership model, no dependency resolution, and no
language server. The formatter does not wrap long expressions yet. The directories for these carry READMEs with the
design constraints already settled; see [docs/roadmap.md](docs/roadmap.md).
