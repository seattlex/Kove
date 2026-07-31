# Diagnostic codes

Every Kove diagnostic carries a stable code: `E....` for errors and
`W....` for warnings. Codes are never reused or renumbered, so
documentation and searches can rely on them.

Each code has a longer explanation than fits on the diagnostic:

```console
$ kove explain E0012
E0012: mismatched types

An expression has one type where another was required.
...
```

The explanations live in `compiler/diagnostics/src/codes.rs`, and tests
check them against the tables below in both directions, so this page and
the compiler cannot drift apart. The
rendering format is part of the compiler's contract and golden-tested:

```text
error[E0012]: mismatched types: expected `Int`, found `String`
 --> src/main.kov:2:20
  |
2 |     let age: Int = "sixteen";
  |                    ^^^^^^^^^ expected `Int`
  |
help: remove the quotes or change the variable type
```

The parts: severity and code; `file:line:column` (1-based, columns count
characters); the source line; a caret marker under the exact span, with
an optional label; then optional `help:` (a suggestion) and `note:`
(background) lines.

## Syntax and literal errors (E00xx, E01xx)

| Code | Meaning |
| --- | --- |
| E0001 | Unrecognized character, not part of any Kove token. |
| E0101 | Expected a specific token (missing `;`, `)`, `}`, `,`, ...). Recovery inserts it and continues, so one typo yields one error. |
| E0102 | Expected an expression (a dangling operator, for example). |
| E0103 | Unexpected input the parser had to skip (an error island). |
| E0104 | A statement at the top level of a file. Code must live inside a function. |
| E0110 | Integer literal doesn't fit in `Int` (64-bit signed). |
| E0111 | Unknown escape sequence in a string or char literal. |
| E0112 | Unterminated string literal. |
| E0113 | Unterminated character literal. |
| E0114 | Unterminated block comment. |

## Semantic errors (E0012, E02xx)

Two stages produce these. `compiler/resolver` owns the ones about
*names* (E0200 to E0202, E0204, E0205, E0213, E0216, E0217, E0219,
E0230); `compiler/typechecker` owns the ones about *types* (E0012,
E0203, E0206 to E0212, E0214, E0215, E0218). A name the resolver could
not resolve becomes an error type, so it never produces a second
diagnostic downstream.

| Code | Meaning |
| --- | --- |
| E0012 | Mismatched types (annotation vs value, argument vs parameter, return vs declared type, assignment vs variable type, range bounds). |
| E0200 | Cannot find a type with this name. |
| E0201 | Cannot find a variable in scope. |
| E0202 | Cannot find a function with this name. |
| E0203 | Wrong number of call arguments. |
| E0204 | Assignment to an immutable variable. Variables need `let mut` to be assignable. |
| E0205 | Duplicate definition (function, type, field, variant, parameter), or a name reserved for a built-in (`println`, `assert`). |
| E0206 | No such field on this struct. |
| E0207 | Struct literal is missing fields. |
| E0208 | Struct literal initializes a field more than once. |
| E0209 | Field access on a value that has no fields. |
| E0210 | A function with a return type has a path that doesn't return. |
| E0211 | An `if`/`while` condition, or an `assert` argument, isn't `Bool`. Kove has no truthiness. |
| E0212 | Operator applied to unsupported operand types (including `Int`/`Float` mixing). |
| E0213 | Invalid assignment target. Only variables and their field chains. |
| E0214 | Missing or malformed `main` (needed by `run` and `build`). |
| E0215 | `println` cannot print this type. |
| E0216 | No such variant on this enum (or the named type has no variants). |
| E0217 | `import` parses but modules aren't implemented yet. |
| E0218 | `for` needs an Int range (`lo..hi`) as its iterable. |
| E0219 | Struct literal syntax used with an enum. |
| E0220 | A `test_` function that `kove test` cannot run (it takes parameters or returns a value). |
| E0230 | Only named functions can be called (no methods or function values yet). |

## Warnings (W00xx)

Warnings are printed like errors but with `-` markers instead of `^`, and
they never fail a build or stop a later compiler stage. Exit codes are
unaffected.

| Code | Meaning |
| --- | --- |
| W0001 | A binding (a `let`, a parameter, or a `for` variable) whose value is never read, including one that is only ever assigned to. Prefix the name with `_` to say it is deliberate. |
| W0002 | A function no execution can reach from `main` or a test. Recursion does not make a function reachable. Quiet in a file that has no entry point at all. Prefix the name with `_` to say it is deliberate. |

## Runtime errors (E03xx)

Reported by the interpreter with the span of the failing operation.

| Code | Meaning |
| --- | --- |
| E0301 | Division by zero (`Int`). |
| E0302 | `Int` arithmetic overflow (checked, never wraps silently). |
| E0303 | Remainder by zero (`Int`). |
| E0304 | Recursion limit (1000 nested calls) exceeded. |
| E0305 | Writing program output failed. |
| E0306 | An `assert` condition was false. |

`Float` follows IEEE 754 and doesn't raise E03xx errors.

## Adding a code

Pick the next free number in the right band, then in the same change:

1. Add it to the table above.
2. Add a `CodeInfo` entry in `compiler/diagnostics/src/codes.rs` with a
   summary and an explanation.
3. Add a test in the suite for the stage that emits it.

The registry tests fail if steps 1 and 2 disagree, so the documentation
and the compiler cannot drift.
