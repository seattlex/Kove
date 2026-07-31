# compiler/hir

**Status:** not implemented. Lands in v0.6, before the backend.

HIR (high-level intermediate representation) is the desugared,
fully-resolved form of a program. The AST mirrors what the user wrote;
HIR mirrors what the program *means*.

## Why it exists

Today the interpreter walks the AST directly and consults the resolver's
maps as it goes. That works for a tree-walker, but a code generator
wants one structure that already has every answer attached, and it wants
the number of distinct constructs to be small.

HIR is where that happens:

- Names are already bound. A reference *is* a `LocalId` or a `FuncId`,
  not a string that needs a lookup.
- Types are already assigned. Every expression carries its `Ty`.
- Sugar is gone. `for i in 0..n { ... }` becomes a `while` over a
  counter; `else if` chains become nested ifs. The backend sees fewer
  shapes than the parser accepts.

## Constraints already decided

- Spans survive into HIR. Runtime diagnostics (division by zero,
  overflow) point at source today and must keep doing so.
- HIR is built from the AST plus [`kove_resolver::Resolutions`] and the
  type checker's results, so nothing here re-derives what earlier stages
  already know. That is the same rule the resolver/type-checker split
  follows.
- The interpreter moves onto HIR when HIR exists, so there is one
  execution semantics, not two that can drift.

## Not decided yet

Whether HIR is a tree or already in a form indexed by id (an arena).
Leaning arena, because MIR construction and the future borrow checker
both want cheap references to sub-expressions.
