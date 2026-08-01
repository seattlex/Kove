# Contributing to Kove

## Getting started

```console
$ cargo build          # builds the whole toolchain; `kove` lands in target/debug
$ cargo test           # the full compiler test suite
$ target/debug/kove run examples/hello.kov
```

The workspace layout and each crate's role are described in
[docs/compiler.md](docs/compiler.md), which also has the step-by-step
checklist for adding a language feature.

## Direction

[docs/north-star.md](docs/north-star.md) is the tiebreaker for design
questions: Rust's safety, Go's simplicity, Zig's tooling, self-hosted in
the long term. When proposing a change, say which clause it serves and
what it costs the others. [docs/roadmap.md](docs/roadmap.md) says what
version the work belongs to.

## Engineering principles

These are the project's standing rules. Changes are reviewed against
them.

- Don't prematurely optimize. Correctness and architecture come first.
- No syntax without documented semantics. If it parses, its behavior is
  written down in [docs/language.md](docs/language.md) and
  [docs/syntax.md](docs/syntax.md) in the same change.
- Never duplicate compiler logic. There is one token vocabulary
  (`compiler/lexer`) and one grammar (`compiler/parser`), and they serve
  the compiler, the future formatter and the future LSP alike. The same
  goes for every later stage.
- Preserve source spans throughout compilation. Every AST node and
  diagnostic points at real bytes; nothing may drop spans.
- Prefer explicit representations over clever abstractions.
- Keep compiler phases modular: one crate per stage, testable alone.
  Self-hosting means porting them one at a time, so a stage that cannot
  be understood alone cannot be ported alone.
- A stage never redoes an earlier stage's work. The type checker does
  not resolve names; the backend will not re-derive types. If you need
  something an earlier stage knew, have it record the answer.
- Write tests alongside each feature, including intentionally invalid
  programs. New diagnostics get a code in
  [docs/diagnostics.md](docs/diagnostics.md) and a test asserting it.
- Don't silently change language semantics. Behavior changes are called
  out, documented, and reflected in tests.
- Report multiple errors when practical. Recovery in the parser and
  `Ty::Error` in the checker exist so one mistake never hides another,
  and one mistake never causes a cascade either.

Kove is designed as a language ecosystem, not merely an interpreter.
Milestones are small, but every piece is built so the complete vision
(native backend, package manager, LSP) has room to land.

## Commit signing

Commits should be signed. Kove uses SSH signing, so the key you already
push with can sign too:

```console
$ git config gpg.format ssh
$ git config user.signingkey ~/.ssh/id_ed25519.pub
$ git config commit.gpgsign true
```

To verify signatures locally, point git at the repository's list of
trusted keys:

```console
$ git config gpg.ssh.allowedSignersFile .allowed_signers
$ git log --show-signature -1
```

Add your public key to [.allowed_signers](.allowed_signers) in the same
change as your first signed commit. For GitHub to show a commit as
Verified, the same public key also has to be registered on your account
as a *signing* key, which is a separate entry from an authentication
key.

Two things that waste an afternoon:

- **"Key is already in use"** when adding the key on GitHub means it is
  already there as an authentication key. The form defaults to that
  type; change **Key type** to *Signing Key* before pasting. The same
  key is allowed to be both.
- **`git log --format=%G?` printing `B`** does not mean the signature is
  bad. Git verifies SSH signatures by shelling out to `ssh-keygen`, so
  it reports `B` when `ssh-keygen` is missing or when `gpg.ssh.program`
  points at something that cannot verify. Check that
  `ssh-keygen -Y find-principals` runs before believing it. `U` is
  different and does mean something: the signature is good but the key
  is not in `.allowed_signers`.

To sign commits you already made, amend them in a rebase rather than one
at a time:

```console
$ git rebase --committer-date-is-author-date \
    --exec 'git commit --amend --no-edit -S' <base>
```

## Diagnostics style

Diagnostics are a feature, with a golden-tested format. When writing
one: say what went wrong in the message, point the span at the exact
offender, use the label for what was expected here, and reserve `help:`
for a concrete action the user can take. See
[docs/diagnostics.md](docs/diagnostics.md).

## Tests

One suite per compiler stage under `tests/tests/`, fixture programs
under `tests/programs/` (`valid/` with `.stdout` twins, `invalid/` with
`// expect: E0012` markers, `runtime/` with `// expect-runtime:`).
A new fixture pair is the cheapest meaningful test, so prefer adding one
over not testing.
