# Kove

Kove is a statically typed, compiled programming language designed as a
safe, self-contained language ecosystem. The name is a stylized spelling
of *cove*: a contained, sheltered environment — the language ships with
its own coherent toolchain instead of leaning on external tooling.

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

Kove is at the **first milestone**: a working frontend (lexer, parser,
AST, type checker), a reference interpreter, and a `kove` CLI — with the
diagnostics quality and testing discipline the rest of the project will
build on. It is a young language; the roadmap below is explicit about
what exists and what doesn't.

| Phase | | |
| --- | --- | --- |
| 1 | Lexer | ✅ (via [ReParse]) |
| 2 | Parser | ✅ (via [ReParse], with error recovery) |
| 3 | AST with spans | ✅ |
| 4 | Type checker | ✅ |
| 5 | Interpreter (`kove run`) | ✅ |
| 6 | Intermediate representation | — |
| 7 | Native backend (x86-64 Linux first) | — |
| 8 | Standard library | — |
| 9 | Package manager | — |
| 10 | LSP + formatter | — (the frontend is already LSP-ready) |

The frontend runs on [ReParse], our incremental parser engine: the same
grammar definition powers batch compilation today and will power the
language server later — incremental reparsing, error recovery (any input
produces a complete tree), syntax highlighting and document symbols come
with it. The compiler and the LSP can never disagree about syntax
because there is only one parser.

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

Commands: `kove build`, `kove run`, `kove check`, `kove fmt` (reserved),
`kove version`. When the file argument is omitted, `src/main.kov` then
`main.kov` are tried. Exit codes are stable for CI: `0` success, `1` the
program has errors, `2` the CLI was misused.

`kove build` currently stops after a full compile check — native code
generation is roadmap phase 7. `kove run` executes through the reference
interpreter.

## Diagnostics

Compiler errors are a design priority, not an afterthought: every error
has a stable code, a source snippet with a caret marker, and — where the
compiler can tell what you meant — a suggestion:

```text
error[E0012]: mismatched types: expected `Int`, found `String`
 --> src/main.kov:2:20
  |
2 |     let age: Int = "sixteen";
  |                    ^^^^^^^^^ expected `Int`
  |
help: remove the quotes or change the variable type
```

The full code registry is in [docs/diagnostics.md](docs/diagnostics.md),
and the renderer's format is golden-tested.

## Documentation

- [docs/language.md](docs/language.md) — the language as it exists today,
  with semantics
- [docs/syntax.md](docs/syntax.md) — grammar reference
- [docs/compiler.md](docs/compiler.md) — pipeline and crate architecture
- [docs/diagnostics.md](docs/diagnostics.md) — error code registry
- [docs/ownership.md](docs/ownership.md) — the memory model today and the
  ownership design ahead
- [CONTRIBUTING.md](CONTRIBUTING.md) — engineering principles and how to
  work on the compiler

## Repository layout

```text
kove/
├── compiler/
│   ├── syntax/        the Kove grammar on ReParse (lexer + parser + CST)
│   ├── ast/           AST types and CST → AST lowering
│   ├── typechecker/   name resolution and type checking
│   └── diagnostics/   diagnostic types and the terminal renderer
├── runtime/
│   └── interpreter/   the tree-walking reference interpreter
├── cli/               the `kove` binary and the compiler driver
├── tests/             lexer / parser / ast / typecheck / diagnostics /
│                      compiler / integration suites + fixture programs
├── docs/
└── examples/
```
