# Compiler architecture

## Pipeline

```text
.kov source
    │
    ▼
Lexer ─────────────┐
    │              │
    ▼              │ compiler/syntax — the Kove grammar on the
Parser             │ ReParse engine: tokens, rules, recovery,
    │              │ highlighting/symbol/folding annotations
    ▼              │
Concrete tree ─────┘
    │
    ▼
AST ──────────────── compiler/ast — span-preserving AST + lowering
    │
    ▼
Name resolution ───┐
    │              │ compiler/typechecker
    ▼              │
Type checking ─────┘
    │
    ▼
Interpreter ──────── runtime/interpreter (roadmap: HIR → MIR → native
                     backend take over from here)
```

Every stage is a separate crate with its own tests, and every stage
preserves byte spans so diagnostics always point at source.

## Crates

| Crate | Role |
| --- | --- |
| `kove-diagnostics` | `Span`, `Diagnostic` (code, message, label, help, notes), `SourceFile`, and the terminal renderer. Depended on by everything; depends on nothing. |
| `kove-syntax` | The grammar definition and the mapping from ReParse recovery diagnostics to Kove error codes. Re-exports `reparse`. |
| `kove-ast` | AST types and the lowering pass. Lowering never fails: malformed subtrees become `ExprKind::Error` placeholders. Also decodes literals (escapes, overflow) — the only diagnostics this stage owns. |
| `kove-typechecker` | Two passes: collect item signatures (so items forward-reference freely), then check bodies. One mistake produces one error: failed expressions get `Ty::Error`, which unifies with everything and stops cascades. |
| `kove-interpreter` | Tree-walking evaluator. Assumes a type-checked program; typing violations are internal compiler errors (panics), while arithmetic safety (E03xx) is checked and reported with spans. |
| `kove-cli` | The driver (`compile`, `compile_executable`, `run`) as a library, plus the `kove` binary. |
| `kove-tests` | The cross-cutting test suite (see below). |

## Why ReParse

The frontend runs on [ReParse](https://github.com/seattlex/ReParse),
our incremental parser engine (green/red trees, memoizing PEG with
exact examined-extent tracking). What Kove gets from it:

- **One frontend for everything.** The requirements are explicit that
  the LSP must reuse the compiler frontend rather than a second parser.
  ReParse trees are LSP-shaped by design (incremental reparsing, error
  recovery, highlighting, symbols, folding are annotations on the same
  grammar), so the future `kove-lsp` consumes exactly what `kove build`
  consumes.
- **Recovery for free, everywhere.** Any input produces a complete
  tree; the compiler reports multiple independent syntax errors and
  the type checker is never exposed to a half-tree.
- **A CST/AST split.** The concrete tree preserves every byte (good for
  tooling and the future formatter); `kove-ast` lowers it to a clean
  AST for the semantic stages.

The dependency is pinned by revision in the workspace `Cargo.toml` for
reproducible builds.

## Driver policy

- Phases run in order: syntax → lowering (literal decoding) → type
  checking. If the syntax phase reports anything, type checking is
  skipped — semantic errors on top of a broken parse are noise.
  Within a phase, every error found is reported, sorted by source
  position.
- `kove check` runs the frontend. `kove run`/`kove build` additionally
  require a well-formed `main` (E0214); `run` then interprets, `build`
  stops with an honest note until the native backend (phase 7) exists.
- The interpreter runs on a dedicated thread with a large fixed stack,
  so Kove's own recursion limit (E0304, 1000 frames) is what stops
  runaway recursion — never a host stack overflow.
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
tests/tests/diagnostics.rs   golden rendered output (format is contract)
tests/tests/compiler.rs      driver policy + runtime error codes
tests/tests/integration.rs   fixture programs in tests/programs/
cli/tests/cli.rs             the binary: commands, exit codes, stdio
```

Fixture convention: `tests/programs/valid/*.kov` runs and must match
its `.stdout` twin; `invalid/*.kov` declares `// expect: E0012` lines;
`runtime/*.kov` declares `// expect-runtime: E0301`. Adding a fixture
pair is the cheapest way to lock in new behavior.

## Adding a language feature

1. Grammar first: extend `compiler/syntax`, add parser golden tests.
2. AST: extend the types and lowering, with tests.
3. Type checking: new rules and any new error codes — register codes in
   `docs/diagnostics.md`, test each.
4. Interpreter: evaluation + runtime tests.
5. Document semantics in `docs/language.md` and the grammar in
   `docs/syntax.md`; add an example if user-visible.

No step is optional; a feature without documented semantics or tests
does not merge.
