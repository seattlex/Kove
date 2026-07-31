# North Star

> Kove is a modern systems programming language that combines Rust's
> safety, Go's simplicity, and Zig's straightforward tooling, while
> remaining entirely self-hosted in the long term.

That sentence is the tiebreaker. When a design question has two
defensible answers, the one that serves this statement wins. What
follows is what each clause actually commits us to, so the statement can
be applied rather than admired.

## Rust's safety

Compile-time guarantees over runtime surprises. Memory safety without a
garbage collector, enforced by the type system, with `unsafe` as the
explicit and enumerated escape hatch.

What it has already decided:

- Immutable by default. Mutation is opt-in and visible at the
  declaration.
- No implicit conversions, not even Int to Float. A widening that is
  usually harmless is still a rule the reader has to know.
- No truthiness. Conditions are `Bool`.
- Int arithmetic is checked. Overflow and division by zero abort with a
  diagnostic rather than wrapping quietly.
- Exhaustiveness, when pattern matching arrives, is checked.

What it does not mean: copying Rust. The ownership model is being
designed deliberately in [ownership.md](ownership.md), and lifetime
syntax in surface Kove is a non-goal for the first iteration.

## Go's simplicity

A language a competent programmer can hold in their head, that reads the
same in every codebase.

What it has already decided:

- One way to write a loop, one way to write a condition, braces
  mandatory.
- An opinionated formatter with no options, so formatting is never
  discussed in review.
- A small standard library that is a compatibility promise, not a
  dumping ground.
- Features earn their place. Generics arrive when the standard library
  cannot be written without them, not because a language ought to have
  them.

The tension with the clause above is real: safety pushes toward more
type-system machinery, simplicity pushes back. Where they conflict,
prefer the rule that is easier to explain in one sentence, and write the
sentence down in [language.md](language.md). If it cannot be explained
in a sentence, that is evidence the design is wrong.

## Zig's straightforward tooling

One binary, no ceremony, nothing to configure before writing code.

What it has already decided:

- `kove new`, `kove build`, `kove run`, `kove check`, `kove fmt`, all in
  one tool with no plugins.
- Diagnostics are a feature: stable codes, source snippets, carets, and
  a suggestion when the compiler can tell what was meant.
- Reproducible builds and a manifest that a person can read.
- Stable exit codes, because tools get scripted.
- No hidden global state. A project is a directory with a `kove.toml`.

## Self-hosted in the long term

The compiler will eventually be written in Kove. That is v1.0 and
beyond, and it is a forcing function rather than a vanity goal: a
language whose compiler cannot be written in it comfortably is missing
something, and self-hosting is how we find out what.

What it decides now, before any of it is written:

- The Rust implementation is a bootstrap, not the definition. The
  language is defined by `docs/` and the test suite, so a second
  implementation has something to conform to.
- Compiler phases stay modular and independently testable, because they
  will be ported one at a time.
- Dependencies are weighed against portability. Every crate the
  bootstrap leans on is something the self-hosted compiler must replace
  or live without, which is a real argument in the backend decision
  (see `compiler/backend/README.md`).
- The language needs to be good at what compilers do: text, trees,
  tagged unions, pattern matching. That is a priority ordering for
  features, not an accident.

## Using this document

In a design discussion, say which clause the change serves and what it
costs the others. A change that serves none of them needs a better
reason than "other languages have it".
