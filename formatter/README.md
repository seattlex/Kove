# formatter

**Status:** implemented. `kove fmt` formats files in place;
`kove fmt --check` reports without writing and exits non-zero if
anything would change.

An opinionated formatter with deterministic output. No options, no style
debates, no configuration file.

## What it decides

- Four-space indentation, one statement per line.
- One space around binary operators; none around `..`, so `0..10` reads
  as one thing.
- One member per line in struct and enum declarations. They are read far
  more often than written, and a diff that touches one field should
  touch one line.
- Struct literals, call arguments and parameter lists stay on one line
  when they fit within 100 columns and contain no comments, and break one
  item per line when they do not. Width decides, not how the author
  happened to type it. A wrapped list gets a trailing comma, so adding an
  item touches one line.
- A single blank line wherever the author left one or more; never two.
- Exactly one trailing newline.

## What it leaves alone

- **Redundant parentheses.** Removing them means reasoning about
  precedence, and a formatter that can change how an expression groups
  is one nobody should trust.
- **Comment placement.** A comment on its own line stays on its own
  line; a comment trailing code stays on that line; an inline block
  comment stays inline.
- **Files that do not parse.** `kove fmt` reports the syntax errors and
  leaves the file untouched. The tree would be complete enough to walk,
  but rewriting a file the compiler rejects is not a formatter's call.

## Why the CST makes this work

The parser is built on ReParse, whose trees are full-fidelity: every
byte of the source is in the tree, comments and whitespace included, and
`tree.width() == text.len()` for any input. Comments attach as trivia to
the following token, which is what lets them be placed deliberately
rather than lost.

One implementation detail carries the comment handling: line breaks are
*pending* rather than written immediately. When a trailing comment shows
up in the next token's leading trivia, the newline the structure asked
for has not been committed yet, so it can still be taken back and the
comment lands on the line it was written on.

## Guarantees, and how they are tested

- **Idempotent.** Formatting formatted code changes nothing. Checked on
  every construct and on every `.kov` file in the repository.
- **Meaning-preserving.** Formatting never changes the token stream, so
  it moves whitespace and nothing else. The single exception is a
  trailing separator (`struct S { a: Int, }`), which is punctuation with
  no meaning and is normalized away.

CI runs `kove fmt --check` over the repository's Kove sources.

## Not yet

- Long *expressions* are not broken up. A single arithmetic or logical
  expression that runs past the width stays on its line; only lists wrap.
- The language server will share this code when it lands, per the rule
  that there is one implementation of everything.
