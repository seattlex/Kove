# Changelog

Notable changes to the Kove toolchain. The language is pre-1.0, so
anything documented here can still change; entries call out when
behavior does.

The format loosely follows [Keep a Changelog](https://keepachangelog.com),
and versions follow [semantic versioning](https://semver.org) once 0.1.0
ships.

## Unreleased (0.1.0-dev)

The first working milestone: roadmap phases 1 to 5, plus the start of
phase 9.

### Language

- Variables with `let`, immutable by default, `let mut` to opt in
- Primitive types: `Int`, `Float`, `Bool`, `Char`, `String`
- Functions with mandatory parameter types and optional return types,
  declared in any order, recursion supported
- `if` / `else if` / `else`, `while`, and `for` over half-open Int
  ranges (`0..10`)
- Structs with literals, field access and field assignment
- Enums with unit variants, `Enum::Variant` paths and equality
- `println` for the primitive types
- Line and block comments
- `import` and `match` reserved, with `import` reporting E0217 so the
  single-file model is never mistaken for the module system to come

### Compiler

- Frontend built on [ReParse](https://github.com/seattlex/ReParse): one
  grammar gives lexing, parsing, error recovery and LSP-ready trees, so
  the compiler and the future language server cannot disagree
- Span-preserving AST with a lowering pass independent of the concrete
  tree
- Two-pass name resolution and type checking, with `Ty::Error` stopping
  error cascades so one mistake produces one diagnostic
- Roughly 30 stable diagnostic codes, rendered with a source snippet,
  caret markers, labels, help and notes
- Tree-walking reference interpreter with checked Int arithmetic
  (division by zero, remainder by zero, overflow) and a recursion limit
  that reports a diagnostic instead of overflowing the host stack
- Value semantics throughout, the deliberate v0.1 memory model

### Tooling

- `kove new` scaffolds a project; `kove build`, `kove run` and
  `kove check` resolve the current project when given no file
- `kove.toml` manifests, parsed strictly with line-numbered errors
- Stable exit codes: 0 success, 1 program errors, 2 CLI misuse
- `kove fmt` is reserved and reports that it is not implemented

### Known gaps

No intermediate representation or native backend yet, so `kove build`
stops after checking and `kove run` interprets. No standard library, no
modules, no pattern matching, no generics or enum payloads, no
references or ownership model, no dependency resolution, no formatter
and no language server. See the roadmap in the README.
