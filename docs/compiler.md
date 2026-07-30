# Compiler architecture

## Pipeline

```text
.kov source
    │
    ▼
Lexer ─────────────┐
    │              │
    ▼              │ compiler/syntax: the Kove grammar on the
Parser             │ ReParse engine. Tokens, rules, recovery,
    │              │ highlighting/symbol/folding annotations.
    ▼              │
Concrete tree ─────┘
    │
    ▼
AST ──────────────── compiler/ast: span-preserving AST + lowering
    │
    ▼
Name resolution ───┐
    │              │ compiler/typechecker
    ▼              │
Type checking ─────┘
    │
    ▼
Interpreter ──────── runtime/interpreter (roadmap: HIR -> MIR -> native
                     backend take over from here)
```

Every stage is a separate crate with its own tests, and every stage
preserves byte spans so diagnostics always point at source.

## Crates

| Crate | Role |
| --- | --- |
| `kove-diagnostics` | `Span`, `Diagnostic` (code, message, label, help, notes), `SourceFile`, and the terminal renderer. Everything depends on it; it depends on nothing. |
| `kove-syntax` | The grammar definition, plus the mapping from ReParse recovery diagnostics to Kove error codes. Re-exports `reparse`. |
| `kove-ast` | AST types and the lowering pass. Lowering never fails: malformed subtrees become `ExprKind::Error` placeholders. Also decodes literals (escapes, overflow), the only diagnostics this stage owns. |
| `kove-typechecker` | Two passes: collect item signatures (so items forward-reference freely), then check bodies. One mistake produces one error: failed expressions get `Ty::Error`, which unifies with everything and stops cascades. |
| `kove-interpreter` | Tree-walking evaluator. Assumes a type-checked program, so typing violations are internal compiler errors (panics), while arithmetic safety (E03xx) is checked and reported with spans. |
| `kove-cli` | The driver (`compile`, `compile_executable`, `run`) as a library, project files (kove.toml, `kove new`), and the `kove` binary. |
| `kove-tests` | The cross-cutting test suite (see below). |

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

- Phases run in order: syntax, then lowering (literal decoding), then
  type checking. If the syntax phase reports anything, type checking is
  skipped, since semantic errors on top of a broken parse are noise.
  Within a phase, every error found is reported, sorted by position.
- `kove check` runs the frontend. `kove run` and `kove build` also
  require a well-formed `main` (E0214). `run` then interprets; `build`
  stops with an honest note until the native backend (phase 7) exists.
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
tests/tests/typecheck.rs     one test per error code + clean programs
tests/tests/diagnostics.rs   golden rendered output (the format is a contract)
tests/tests/compiler.rs      driver policy + runtime error codes
tests/tests/integration.rs   fixture programs in tests/programs/
cli/tests/cli.rs             the binary: commands, exit codes, stdio, projects
```

Fixture convention: `tests/programs/valid/*.kov` runs and must match
its `.stdout` twin; `invalid/*.kov` declares `// expect: E0012` lines;
`runtime/*.kov` declares `// expect-runtime: E0301`. Adding a fixture
pair is the cheapest way to lock in new behavior.

## Adding a language feature

1. Grammar first: extend `compiler/syntax`, add parser golden tests.
2. AST: extend the types and lowering, with tests.
3. Type checking: new rules and any new error codes. Register codes in
   `docs/diagnostics.md` and test each one.
4. Interpreter: evaluation plus runtime tests.
5. Document the semantics in `docs/language.md` and the grammar in
   `docs/syntax.md`; add an example if it's user-visible.

No step is optional. A feature without documented semantics or tests
doesn't merge.
