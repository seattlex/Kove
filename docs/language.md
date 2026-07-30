# The Kove language

This document describes the language as implemented today. Nothing is
documented here that the compiler does not enforce, and no syntax exists
without documented semantics. Planned-but-unimplemented features are in
the final section.

A Kove program is currently a single `.kov` file containing items:
function, struct and enum declarations. Execution starts at
`fn main()` (no parameters, no return type).

## Variables

```kov
let name = "Kove";
let age: Int = 16;
```

- Variables are **immutable by default**. Assigning to one is error
  E0204.
- `let mut` makes a variable mutable:

  ```kov
  let mut counter = 0;
  counter = counter + 1;
  ```

- The type annotation is optional; without one the variable takes the
  type of its initializer. With one, the initializer must match exactly
  (E0012) — there are no implicit conversions.
- A variable keeps its type for its whole life; assignments must match
  it (E0012).
- Shadowing is allowed: a new `let` with the same name creates a new
  variable, and inner scopes hide outer ones. A variable exists from its
  `let` to the end of the enclosing block.

## Types

Primitives:

| Type | Values | Notes |
| --- | --- | --- |
| `Int` | 64-bit signed integers | arithmetic is overflow-checked at runtime (E0302) |
| `Float` | 64-bit IEEE 754 | follows IEEE semantics: `1.0 / 0.0` is `inf`, no overflow errors |
| `Bool` | `true`, `false` | conditions must be `Bool`; there is no truthiness (E0211) |
| `Char` | one Unicode scalar | written `'k'`, `'\n'` |
| `String` | immutable UTF-8 text | written `"text"` |

`Int` and `Float` never mix implicitly (E0212): `1 + 1.5` is an error;
write `1.0 + 1.5`. Sized types (`Int32`, `Float32`, ...) are reserved
for the future.

## Functions

```kov
fn add(a: Int, b: Int) -> Int {
    return a + b;
}
```

- Parameter types are mandatory. The return type is optional; a function
  without `->` returns nothing.
- Arguments are passed **by value** (see [ownership.md](ownership.md)).
- Parameters are immutable.
- A function with a return type must return on every path — the checker
  verifies this conservatively (E0210): a path counts as returning if it
  ends in `return`, or in an `if`/`else if`/`else` chain whose branches
  all return. Loop bodies never count, since they may not run.
- Functions may be declared in any order; recursion (including mutual
  recursion) works.

## Statements and control flow

```kov
if age >= 18 {
    println("Adult");
} else if age >= 13 {
    println("Teenager");
} else {
    println("Minor");
}

while condition {
    // ...
}

for i in 0..10 {
    println(i);
}
```

- Braces are mandatory; conditions take no parentheses.
- `for` iterates over **Int ranges** only for now (E0218). `lo..hi` is
  half-open: `0..3` visits `0, 1, 2`; an empty or reversed range runs
  zero times. The loop variable is a fresh immutable `Int` each
  iteration. Iterating over collections is planned.
- A bare block `{ ... }` is a statement and opens a scope.
- Assignment (`x = ...`, `user.age = ...`) is a statement, not an
  expression. Targets are a variable or a field chain rooted at a
  variable (E0213); the root variable must be `mut` (E0204).

## Expressions

Binary operators, loosest to tightest:

| Precedence | Operators | On | Result |
| --- | --- | --- | --- |
| 1 (loosest) | `..` | `Int`, `Int` | range (for `for` loops) |
| 2 | `\|\|` | `Bool` | `Bool`, short-circuits |
| 3 | `&&` | `Bool` | `Bool`, short-circuits |
| 4 | `==` `!=` | two equal comparable types | `Bool` |
| 5 | `<` `<=` `>` `>=` | two `Int`s or two `Float`s | `Bool` |
| 6 | `+` `-` | two `Int`s or two `Float`s | operand type |
| 7 (tightest) | `*` `/` `%` | two `Int`s or two `Float`s | operand type |

All binary operators associate left. Unary operators are `-` (Int,
Float) and `!` (Bool). `==`/`!=` work on `Int`, `Float`, `Bool`,
`Char`, `String` and enums; comparing structs is not supported yet
(E0212).

Runtime arithmetic on `Int` is checked: division or remainder by zero
stops the program (E0301/E0303), as does overflow (E0302). `Float`
follows IEEE 754 and never traps.

## Structs

```kov
struct User {
    name: String,
    age: Int
}

let user = User {
    name: "Alex",
    age: 20
};

println(user.name);
```

- A struct literal must initialize **every** field, each exactly once
  (E0207, E0208).
- Struct values copy on assignment and on call — mutating a copy never
  affects the original.
- Field access and field assignment work through chains
  (`a.b.c = ...`).
- A struct literal needs at least one field; empty structs cannot be
  constructed yet.

## Enums

```kov
enum Status {
    Active,
    Banned
}

let status = Status::Active;
if status == Status::Banned { ... }
```

Variants are plain (unit) values for now; variants with associated data
(`Ok(T)`) and pattern matching over them are the reason `match` is
already a reserved word.

## Printing

`println(value)` is the built-in output function: it prints one `Int`,
`Float`, `Bool`, `Char` or `String` followed by a newline (E0215 for
anything else). `Float`s print in shortest form: `2.0 / 1.0` prints
`2`.

## Comments

```kov
// line comment
/* block comment */
```

Block comments do not nest. An unterminated block comment is an error
(E0114), not a silent comment-to-end-of-file.

## Reserved words

`fn let mut return if else while for in struct enum import true false
match`

`import` parses (`import std::io;`) but modules are not implemented
yet — using it is error E0217 so nobody mistakes the current
single-file model for the module system to come. `match` has no grammar
at all yet.

## Not in the language yet

Planned and tracked, in rough order: an intermediate representation and
native compilation, a standard library, modules, pattern matching with
exhaustiveness checking, enum payloads and generics
(`Result<T, E>`), collections and `for` over them, references
(`&data`, `&mut data`) under a real ownership model
([ownership.md](ownership.md)), the package manager, formatter and LSP.
