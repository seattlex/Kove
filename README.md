# Kove

> Kove is a modern systems programming language that combines Rust's
> safety, Go's simplicity, and Zig's straightforward tooling, while
> remaining entirely self-hosted in the long term.

The name is a stylized spelling of "cove": a contained, sheltered
environment. The language ships with its own coherent toolchain rather
than leaning on external tools.

Source files use the `.kov` extension:

```kov
fn add(a: Int, b: Int) -> Int {
    return a + b;
}

fn main() {
    let x = add(10, 20);

    if x > 20 {
        println("Greater than twenty");
    }
}
```

## Status: v0.5

A Kove program can be written, checked with real diagnostics, and run.
Native compilation is the next substantial piece of work. The full
[roadmap](docs/roadmap.md) has the detail; the short version:

| | Version | State |
| --- | --- | --- |
| ✅ | v0.1 Tokenizer | done |
| ✅ | v0.2 Parser | done |
| ✅ | v0.3 Semantic analysis | done |
| ✅ | v0.4 Type checker | done |
| ✅ | v0.5 Interpreter | done |
| ▢ | v0.6 Native compiler | not started |
| ◐ | v0.7 Package manager, formatter, LSP | formatter and `kove new` done |
| ▢ | v1.0 Self-hosting begins | not started |

Directories for the parts that do not exist yet are not empty: each one
carries a README with the constraints already decided and the questions
still open, so the next piece of work starts from a design rather than a
blank page.

The frontend runs on [ReParse], our incremental parser engine. One
grammar definition powers batch compilation today and will power the
language server later, so the compiler and the LSP can never disagree
about syntax. Incremental reparsing, error recovery (any input produces
a complete tree), syntax highlighting and document symbols all come from
that same grammar.

[ReParse]: https://github.com/seattlex/ReParse

## Building and using

The toolchain is written in Rust:

```console
$ cargo build --release
$ target/release/kove run examples/hello.kov
Hello, Kove!

$ target/release/kove check examples/structs.kov
checked `examples/structs.kov`: no errors found

$ cargo test          # the whole compiler test suite
```

Starting a project:

```console
$ kove new my_project
created `my_project`
  my_project/kove.toml
  my_project/src/main.kov

next: cd my_project && kove run

$ cd my_project && kove run
Hello from my_project!
```

```toml
# kove.toml
[package]
name = "my_project"
version = "0.1.0"

[dependencies]
```

Commands: `kove new`, `kove build`, `kove run`, `kove check`,
`kove test`, `kove fmt`, `kove explain`, `kove version`. Without a file
argument, kove uses the current project's `src/main.kov`. Exit codes are
stable for CI: `0` success, `1` the program has errors, `2` the CLI was
misused.

Tests are functions named `test_...`, run by `kove test`:

```console
$ kove test examples/tests.kov
running 3 test(s) in `examples/tests.kov`
  ok    test_double
  ok    test_clamp_inside_the_range
  ok    test_clamp_at_the_edges

3 passed
```

Formatting is opinionated and has no options:

```console
$ kove fmt              # rewrite the project's sources
$ kove fmt --check      # report what would change, exit 1 if anything would
```

`kove build` currently stops after a full compile check. Native code
generation is v0.6, and `kove run` executes through the reference
interpreter until it lands.

## Diagnostics

Compiler errors are a design priority. Every error has a stable code, a
source snippet with a caret marker, and a suggestion where the compiler
can tell what you meant:

```text
error[E0012]: mismatched types: expected `Int`, found `String`
 --> src/main.kov:2:20
  |
2 |     let age: Int = "sixteen";
  |                    ^^^^^^^^^ expected `Int`
  |
help: remove the quotes or change the variable type
```

Where the compiler can guess what you meant, it says so:

```text
error[E0201]: cannot find variable `lenght`
 --> src/main.kov:3:13
  |
3 |     println(lenght);
  |             ^^^^^^ not found in this scope
  |
help: did you mean `length`?
```

Every code has a longer explanation behind it:

```console
$ kove explain E0012
E0012: mismatched types

An expression has one type where another was required.
...
```

Warnings are separate from errors: they are reported but never fail a
build. The full code registry is in
[docs/diagnostics.md](docs/diagnostics.md). The output format is
golden-tested, so it's a contract, not an accident.

## Documentation

- [docs/language.md](docs/language.md) - the language as it exists
  today, with semantics
- [docs/syntax.md](docs/syntax.md) - grammar reference
- [docs/compiler.md](docs/compiler.md) - pipeline and crate architecture
- [docs/diagnostics.md](docs/diagnostics.md) - error code registry
- [docs/ownership.md](docs/ownership.md) - the memory model today and
  the ownership design ahead
- [CONTRIBUTING.md](CONTRIBUTING.md) - engineering principles and how to
  work on the compiler
- [CHANGELOG.md](CHANGELOG.md) - what has landed so far
- [docs/north-star.md](docs/north-star.md) - the one-sentence direction
  and what each clause commits us to
- [docs/roadmap.md](docs/roadmap.md) - versions v0.1 to v1.0

## Repository layout

```text
kove/
├── compiler/
│   ├── lexer/         the token vocabulary
│   ├── parser/        the grammar, recovery, concrete syntax tree
│   ├── ast/           AST types and CST -> AST lowering
│   ├── resolver/      symbol tables, scopes, name resolution
│   ├── typechecker/   types, and only types
│   ├── hir/           desugared + resolved + typed        (v0.6)
│   ├── mir/           basic blocks and control flow       (v0.6)
│   ├── backend/       native code generation              (v0.6)
│   └── diagnostics/   diagnostic types and the renderer
│
├── runtime/
│   └── interpreter/   the tree-walking reference implementation
├── std/               the standard library, in Kove       (v0.6/v0.7)
├── cli/               the `kove` binary and the driver
├── formatter/         `kove fmt`
├── lsp/               the language server                 (v0.7)
├── crates/
│   └── manifest/      kove.toml and project layout
├── tests/             one suite per stage + fixture programs
├── docs/
└── examples/
```

One crate per pipeline stage, each independently testable, because
self-hosting means porting them one at a time. `crates/` holds the
pieces that are not compiler stages and get shared: `manifest` is
needed by the CLI today and by the package manager and language server
later.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option, matching [ReParse](https://github.com/seattlex/ReParse)
so the two projects stay compatible.
