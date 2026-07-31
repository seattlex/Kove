# The Kove language

This describes the language as implemented today. Nothing in here is
aspirational: if it's documented, the compiler enforces it, and no
syntax exists without documented semantics. Planned features are listed
at the end.

A Kove program is currently a single `.kov` file containing items:
function, struct and enum declarations. Execution starts at `fn main()`
(no parameters, no return type).

## Variables

```kov
let name = "Kove";
let age: Int = 16;
```

Variables are immutable by default. Assigning to one is error E0204.
`let mut` makes a variable mutable:

```kov
let mut counter = 0;
counter = counter + 1;
```

The type annotation is optional. Without one the variable takes the type
of its initializer. With one, the initializer has to match exactly
(E0012); there are no implicit conversions. A variable keeps its type
for its whole life, and assignments have to match it.

Shadowing is allowed: a new `let` with the same name creates a new
variable, and inner scopes hide outer ones. A variable exists from its
`let` to the end of the enclosing block.

## Types

| Type | Values | Notes |
| --- | --- | --- |
| `Int` | 64-bit signed integers | arithmetic is overflow-checked at runtime (E0302) |
| `Float` | 64-bit IEEE 754 | IEEE semantics: `1.0 / 0.0` is `inf`, no overflow errors |
| `Bool` | `true`, `false` | conditions must be `Bool`, there is no truthiness (E0211) |
| `Char` | one Unicode scalar | written `'k'`, `'\n'` |
| `String` | immutable UTF-8 text | written `"text"` |

`Int` and `Float` never mix implicitly (E0212). `1 + 1.5` is an error;
write `1.0 + 1.5`. Sized types (`Int32`, `Float32`, ...) are reserved
for later.

## Functions

```kov
fn add(a: Int, b: Int) -> Int {
    return a + b;
}
```

Parameter types are mandatory. The return type is optional, and a
function without `->` returns nothing. Arguments are passed by value
(see [ownership.md](ownership.md)), and parameters are immutable.

A function with a return type must return on every path (E0210). The
check is conservative: a path counts as returning if it ends in
`return`, or in an `if`/`else if`/`else` chain whose branches all
return. Loop bodies never count, since they may not run.

Functions can be declared in any order. Recursion works, including
mutual recursion.

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

Braces are mandatory and conditions take no parentheses.

`for` iterates over Int ranges only for now (E0218). `lo..hi` is
half-open: `0..3` visits `0, 1, 2`, and an empty or reversed range runs
zero times. The loop variable is a fresh immutable `Int` each iteration.
Iterating over collections is planned.

A bare block `{ ... }` is a statement and opens a scope.

Assignment (`x = ...`, `user.age = ...`) is a statement, not an
expression. A target is a variable or a field chain rooted at a variable
(E0213), and the root variable must be `mut` (E0204).

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
Float) and `!` (Bool). `==` and `!=` work on `Int`, `Float`, `Bool`,
`Char`, `String` and enums. Comparing structs isn't supported yet
(E0212).

Runtime arithmetic on `Int` is checked: division or remainder by zero
stops the program (E0301/E0303), and so does overflow (E0302). `Float`
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

A struct literal must initialize every field, each exactly once (E0207,
E0208). Struct values copy on assignment and on call, so mutating a copy
never affects the original. Field access and field assignment work
through chains (`a.b.c = ...`).

A struct literal needs at least one field, so empty structs can't be
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

Variants are plain (unit) values for now. Variants with associated data
(`Ok(T)`) and pattern matching over them are the reason `match` is
already a reserved word.

## Printing

`println(value)` is the built-in output function. It prints one `Int`,
`Float`, `Bool`, `Char` or `String` followed by a newline (E0215 for
anything else). `Float`s print in shortest form: `2.0 / 1.0` prints `2`.

## Comments

```kov
// line comment
/* block comment */
```

Block comments don't nest. An unterminated block comment is an error
(E0114), not a silent comment-to-end-of-file.

## Projects

`kove new my_project` creates a project:

```text
my_project/
├── kove.toml
└── src/
    └── main.kov
```

```toml
[package]
name = "my_project"
version = "0.1.0"

[dependencies]
```

Inside a project, `kove run`, `kove build` and `kove check` need no file
argument. Package names follow identifier rules (letters, digits,
underscores, starting with a letter) because a package will one day be
importable as a module. Dependencies aren't supported yet; listing one
is an error until the package manager lands.

## Reserved words

`fn let mut return if else while for in struct enum import true false
match`

`import` parses (`import std::io;`) but modules aren't implemented yet.
Using it is error E0217, so nobody mistakes the current single-file
model for the module system to come. `match` has no grammar at all yet.

## Not in the language yet

Planned and tracked, in rough order: an intermediate representation and
native compilation, a standard library, modules, pattern matching with
exhaustiveness checking, enum payloads and generics (`Result<T, E>`),
collections and `for` over them, references (`&data`, `&mut data`) under
a real ownership model ([ownership.md](ownership.md)), dependency
resolution for the package manager, the formatter and the LSP.
