# Contributing to Kove

## Getting started

```console
$ cargo build          # builds the whole toolchain; `kove` lands in target/debug
$ cargo test           # the full compiler test suite
$ target/debug/kove run examples/hello.kov
```

The workspace layout and each crate's role are described in
[docs/compiler.md](docs/compiler.md), which also has the step-by-step
checklist for adding a language feature.

## Engineering principles

These are the project's standing rules. Changes are reviewed against
them.

- Don't prematurely optimize. Correctness and architecture come first.
- No syntax without documented semantics. If it parses, its behavior is
  written down in [docs/language.md](docs/language.md) and
  [docs/syntax.md](docs/syntax.md) in the same change.
- Never duplicate compiler logic. There is one grammar
  (`compiler/syntax`) and it serves the compiler and the future LSP
  alike. The same goes for every later stage.
- Preserve source spans throughout compilation. Every AST node and
  diagnostic points at real bytes; nothing may drop spans.
- Prefer explicit representations over clever abstractions.
- Keep compiler phases modular: separate crates, testable alone.
- Write tests alongside each feature, including intentionally invalid
  programs. New diagnostics get a code in
  [docs/diagnostics.md](docs/diagnostics.md) and a test asserting it.
- Don't silently change language semantics. Behavior changes are called
  out, documented, and reflected in tests.
- Report multiple errors when practical. Recovery in the parser and
  `Ty::Error` in the checker exist so one mistake never hides another,
  and one mistake never causes a cascade either.

Kove is designed as a language ecosystem, not merely an interpreter.
Milestones are small, but every piece is built so the complete vision
(native backend, package manager, LSP) has room to land.

## Diagnostics style

Diagnostics are a feature, with a golden-tested format. When writing
one: say what went wrong in the message, point the span at the exact
offender, use the label for what was expected here, and reserve `help:`
for a concrete action the user can take. See
[docs/diagnostics.md](docs/diagnostics.md).

## Tests

One suite per compiler stage under `tests/tests/`, fixture programs
under `tests/programs/` (`valid/` with `.stdout` twins, `invalid/` with
`// expect: E0012` markers, `runtime/` with `// expect-runtime:`).
A new fixture pair is the cheapest meaningful test, so prefer adding one
over not testing.
