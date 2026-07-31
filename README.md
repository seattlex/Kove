# Kove

Kove is a statically typed, compiled programming language built as a
self-contained ecosystem. The name is a stylized spelling of "cove": a
contained, sheltered environment. The point is that the language ships
with its own coherent toolchain instead of leaning on external tools.

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

## Status

Kove is at its first milestone: a working frontend (lexer, parser, AST,
type checker), a reference interpreter, and the `kove` CLI. It's a young
language, and the table below is honest about what exists and what
doesn't.

| Phase | | |
| --- | --- | --- |
| 1 | Lexer | done (via [ReParse]) |
| 2 | Parser | done (via [ReParse], with error recovery) |
| 3 | AST with spans | done |
| 4 | Type checker | done |
| 5 | Interpreter (`kove run`) | done |
| 6 | Intermediate representation | not yet |
| 7 | Native backend (x86-64 Linux first) | not yet |
| 8 | Standard library | not yet |
| 9 | Package manager | started (`kove new` + kove.toml) |
| 10 | LSP + formatter | not yet (the frontend is already LSP-ready) |

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

Commands: `kove new`, `kove build`, `kove run`, `kove check`, `kove fmt`
(reserved), `kove version`. Without a file argument, kove uses the
current project's `src/main.kov`. Exit codes are stable for CI: `0`
success, `1` the program has errors, `2` the CLI was misused.

`kove build` currently stops after a full compile check. Native code
generation is roadmap phase 7, and `kove run` executes through the
reference interpreter until it lands.

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

The full code registry is in [docs/diagnostics.md](docs/diagnostics.md).
The output format is golden-tested, so it's a contract, not an accident.

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

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option, matching [ReParse](https://github.com/seattlex/ReParse)
so the two projects stay compatible.

## Repository layout

```text
kove/
├── compiler/
│   ├── syntax/        the Kove grammar on ReParse (lexer + parser + CST)
│   ├── ast/           AST types and CST -> AST lowering
│   ├── typechecker/   name resolution and type checking
│   └── diagnostics/   diagnostic types and the terminal renderer
├── runtime/
│   └── interpreter/   the tree-walking reference interpreter
├── cli/               the `kove` binary, the driver, project files
├── tests/             lexer / parser / ast / typecheck / diagnostics /
│                      compiler / integration suites + fixture programs
├── docs/
└── examples/
```
