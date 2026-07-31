# formatter

**Status:** not implemented. Lands in v0.7. `kove fmt` exists as a
command and reports that it is unavailable rather than doing nothing.

An opinionated formatter with deterministic output. No options, no
style debates, no configuration file.

## Why it belongs here and not in `compiler/`

The formatter is not a compile stage. It reads the concrete syntax tree
and writes text; it never sees the AST, because the AST has thrown away
exactly what the formatter needs.

## Why the CST makes this tractable

The parser is built on ReParse, whose trees are full-fidelity: every
byte of the source is in the tree, comments and whitespace included, and
`tree.width() == text.len()` for any input at all. A formatter needs
precisely that. It also means:

- Broken code can still be formatted, because broken code still parses
  into a complete tree with error islands.
- Comments have a defined home (they attach as trivia to the following
  token), so they can be moved deliberately rather than lost.

## Constraints already decided

- Formatting is idempotent: formatting formatted code changes nothing.
  This gets a test on every example in the repository.
- Formatting never changes meaning. The token stream after formatting
  must be identical to the token stream before it, comments aside. That
  is checkable, and it will be checked.
- `kove fmt` and the language server's format request share this code.
  One implementation, per the engineering principles.
