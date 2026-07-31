# Roadmap

Kove is built in versions, not in "build a compiler". Each version is a
usable artifact that does something the previous one could not, and each
one ships with tests and documentation before the next begins.

Direction for all of it is [north-star.md](north-star.md).

## Where we are

**v0.5 is complete.** A Kove program can be written, checked with real
diagnostics, and executed. The next substantial piece of work is v0.6,
native compilation, which starts with an intermediate representation.

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

## v0.1 — Tokenizer

- [x] Reads `.kov` files
- [x] Produces tokens with exact source spans
- [x] Good diagnostics: unrecognized characters, unterminated strings,
      chars and block comments, each with its own code
- [x] Longest-match rules pinned by tests (`1..10`, `1.5`, `intx`)

Lives in `compiler/lexer`.

## v0.2 — Parser

- [x] A concrete syntax tree covering every byte of any input
- [x] An AST independent of parser details, with spans preserved
- [x] Syntax errors that recover and keep going, so one typo is one
      error and several mistakes are all reported
- [x] **Pretty printer.** Done, as the formatter: `kove fmt` prints Kove
      source back out from the concrete tree. (A separate s-expression
      dump exists for debugging and golden tests.)

Lives in `compiler/parser` and `compiler/ast`.

## v0.3 — Semantic analysis

- [x] Symbol tables for structs, enums and functions, order-independent
- [x] Name resolution: every reference bound to what it names
- [x] Lexical scopes, including shadowing and per-block lifetimes
- [x] Mutability tracked on bindings
- [x] Diagnostics for unknown and duplicate names, with suggestions for
      near misses
- [x] Lints over the resolver's output: W0001 (a binding whose value is
      never read) and W0002 (a function nothing can reach)

Lives in `compiler/resolver`.

## v0.4 — Type checker

- [x] Variables, with inference from the initializer when unannotated
- [x] Functions: parameters, return types, argument checking,
      all-paths-return
- [x] Structs and enums: literals, fields, variants
- [x] No implicit conversions, no truthiness
- [ ] Inference beyond the local case. There is no unification and no
      generics; both wait for a language feature that needs them.

Lives in `compiler/typechecker`.

## v0.5 — Interpreter

- [x] Executes Kove without producing a binary (`kove run`)
- [x] Checked Int arithmetic and a recursion limit, reported as
      diagnostics with spans
- [x] Value semantics, the deliberate memory model until ownership is
      designed

Lives in `runtime/interpreter`. It stays after the native backend
exists, as the reference implementation the two are tested against.

## v0.6 — Native compiler

The current gap. In order:

- [ ] **HIR** (`compiler/hir`): desugared, resolved, typed
- [ ] **MIR** (`compiler/mir`): basic blocks and explicit control flow
- [ ] **Backend** (`compiler/backend`): x86-64 Linux first
- [ ] `kove build` emits an executable
- [ ] The interpreter and the compiled program agree on every test
      program

Each directory has a README with the constraints already decided and the
open questions, including the Cranelift-versus-LLVM-versus-direct choice
that has to be settled before backend code is written.

Modules and the beginnings of `std` belong here too: a native program
that can only call `println` is not yet a systems language.

## v0.7 — Tooling

- [x] `kove new` and `kove.toml`
- [ ] Dependency resolution, lockfiles, semantic versioning
- [ ] Local and git dependencies (a registry stays a non-goal for now)
- [x] `kove test`, with an `assert` builtin behind it
- [x] **Formatter** (`formatter/`): deterministic, opinionated,
      idempotent, meaning-preserving, enforced in CI. Wraps lists on
      width; does not break up long expressions yet
- [ ] **Language server** (`lsp/`): diagnostics, highlighting, symbols,
      go-to-definition, find references, hover, rename

The frontend was built for both: full-fidelity trees for the formatter,
incremental reparsing and grammar-level editor annotations for the
server. Neither gets a second parser, and the formatter that shipped
reads the same concrete tree the compiler does.

## v1.0 — Self-hosting begins

Not a release, a starting line. Before it makes sense:

- The language must express what a compiler needs: pattern matching
  with exhaustiveness, enums with payloads, collections, generics where
  the standard library demands them
- The ownership model must be designed and implemented
      ([ownership.md](ownership.md))
- The specification and test suite must be complete enough for a second
  implementation to conform to, since that is what the Rust bootstrap
  becomes: an implementation, not the definition

Then the compiler gets ported to Kove one phase at a time, which the
crate boundaries were chosen to allow.
