# Compiler architecture

## Pipeline

```text
.kov source
    │
    ▼
Lexer ────────────── compiler/lexer: the token vocabulary
    │
    ▼
Parser ───────────── compiler/parser: grammar, recovery, editor
    │                annotations
    ▼
Concrete tree
    │
    ▼
AST ──────────────── compiler/ast: span-preserving AST + lowering
    │
    ▼
Name resolution ──── compiler/resolver: symbol tables, scopes, and
    │                every reference bound to what it names
    ▼
Type checking ────── compiler/typechecker: types, and only types
    │
    ▼
Interpreter ──────── runtime/interpreter
                     (v0.6: HIR -> MIR -> backend take over here)
```

One crate per stage, each with its own tests, each preserving byte spans
so diagnostics always point back at source. The boundaries are not
decoration: self-hosting means porting these one at a time, so a stage
that cannot be understood alone is a stage that cannot be ported alone.

## Crates

| Crate | Role |
| --- | --- |
| `kove-diagnostics` | `Span`, `Diagnostic` (code, message, label, help, notes), `SourceFile`, and the terminal renderer. Everything depends on it; it depends on nothing. |
| `kove-lexer` | The token vocabulary: patterns, keywords, trivia, and lexical diagnostics (unterminated literals and comments). Exports a `Tokens` handle and the shared token-name constants, so what counts as a token is defined in exactly one place. |
| `kove-parser` | The grammar: rules, recovery policy, editor annotations, and the mapping from ReParse recovery diagnostics to Kove error codes. Names tokens only through the lexer's handle. |
| `kove-ast` | AST types and the lowering pass. Every node carries a `NodeId`. Lowering never fails: malformed subtrees become `ExprKind::Error` placeholders. Also decodes literals (escapes, overflow), the only diagnostics this stage owns. |
| `kove-resolver` | Symbol tables and lexical scopes. Binds every reference to a local, function or item, and records mutability. Owns the diagnostics about names. |
| `kove-typechecker` | Types only. Consumes the resolver's map, so it never performs a lookup: where the resolver produced a binding, the checker records that binding's type. Failed expressions get `Ty::Error`, which is compatible with everything and stops cascades. |
| `kove-interpreter` | Tree-walking evaluator. Assumes a checked program, so typing violations are internal compiler errors (panics), while arithmetic safety (E03xx) is checked and reported with spans. |
| `kove-manifest` | `kove.toml` and project layout. Outside `compiler/` because it is not a compile stage: it defines what a *package* is, which the CLI needs now and the package manager and language server will need later. |
| `kove-formatter` | `kove fmt`. Reads the concrete syntax tree, not the AST, because the AST has already discarded comments and the author's grouping. Idempotent and token-preserving, both tested. |
| `kove-cli` | The driver (`compile`, `compile_executable`, `run`) as a library, plus the `kove` binary. |
| `kove-tests` | The cross-cutting test suite (see below). |

### Why resolution and type checking are separate

They answer different questions. "Which `count` is this?" is about
scopes and shadowing; "is it an `Int`?" is about types. Fusing them
means every future analysis that needs bindings but not types (the
borrow checker, go-to-definition, rename) either re-implements scope
walking or drags the type checker along.

The seam is `NodeId`. Lowering numbers every AST node; the resolver maps
reference ids to bindings and declaration ids to `LocalId`s; the type
checker maps `LocalId`s to types. Nothing is keyed by name or by span
after lowering, and no stage repeats another's work.

## Why ReParse

The frontend runs on [ReParse](https://github.com/seattlex/ReParse),
our incremental parser engine (green/red trees, memoizing PEG with
exact examined-extent tracking). What Kove gets from it:

- One frontend for everything. The requirements are explicit that the
  LSP must reuse the compiler frontend rather than a second parser.
  ReParse trees are LSP-shaped by design (incremental reparsing, error
  recovery, highlighting, symbols and folding are annotations on the
  same grammar), so the future `kove-lsp` consumes exactly what
  `kove build` consumes.
- Recovery for free, everywhere. Any input produces a complete tree.
  The compiler reports multiple independent syntax errors, and the type
  checker is never exposed to a half-tree.
- A CST/AST split. The concrete tree preserves every byte (good for
  tooling and the future formatter), and `kove-ast` lowers it to a
  clean AST for the semantic stages.

The dependency is pinned by revision in the workspace `Cargo.toml` for
reproducible builds.

## Driver policy

- Phases run in order: syntax, lowering (literal decoding), resolution,
  then type checking. If syntax or lowering reports anything, the
  semantic stages are skipped, since semantic errors on top of a broken
  parse are noise. Resolution and type checking both run and their
  diagnostics are merged, so a file with an unknown name and an
  unrelated type error reports both. Within a phase, every error found
  is reported, sorted by position.
- `kove check` runs the frontend. `kove run` and `kove build` also
  require a well-formed `main` (E0214). `run` then interprets; `build`
  stops with an honest note until the native backend (v0.6) exists.
- Without a file argument, the CLI resolves the current project: a
  `kove.toml` must parse (its errors are CLI errors, exit code 2, not
  compiler diagnostics) and the entry point is `src/main.kov`.
- The interpreter runs on a dedicated thread with a large fixed stack,
  so Kove's own recursion limit (E0304, 1000 frames) is what stops
  runaway recursion, never a host stack overflow.
- Exit codes: `0` success, `1` program errors (compile-time or
  runtime), `2` CLI usage errors. Stable for scripts and CI.

## Testing

`tests/` covers the categories the project requires, one file each:

```text
tests/tests/lexer.rs         token-level behavior and lex diagnostics
tests/tests/parser.rs        golden parse trees, recovery, precedence,
                             incremental-reparse consistency
tests/tests/ast.rs           lowering shapes, spans, literal decoding
tests/tests/resolver.rs      name diagnostics + the resolution map itself
tests/tests/typecheck.rs     one test per type error code + clean programs
tests/tests/diagnostics.rs   golden rendered output (the format is a contract)
tests/tests/compiler.rs      driver policy + runtime error codes
tests/tests/integration.rs   fixture programs in tests/programs/
cli/tests/cli.rs             the binary: commands, exit codes, stdio,
                             projects, formatting
formatter/tests/format.rs    golden output per construct, plus the
                             idempotence and token-preservation properties
```

Fixture convention: `tests/programs/valid/*.kov` runs and must match
its `.stdout` twin; `invalid/*.kov` declares `// expect: E0012` lines;
`runtime/*.kov` declares `// expect-runtime: E0301`. Adding a fixture
pair is the cheapest way to lock in new behavior.

## Adding a language feature

1. Tokens, if any are new: extend `compiler/lexer`, with tests.
2. Grammar: extend `compiler/parser`, add parser golden tests.
3. AST: extend the types and lowering, with tests.
4. Resolution: if the feature introduces or references names, extend
   `compiler/resolver` and test the resulting map, not just the
   diagnostics.
5. Type checking: new rules and any new error codes. Register codes in
   `docs/diagnostics.md` and test each one.
6. Interpreter: evaluation plus runtime tests.
7. Document the semantics in `docs/language.md` and the grammar in
   `docs/syntax.md`; add an example if it's user-visible.

No step is optional. A feature without documented semantics or tests
doesn't merge.
