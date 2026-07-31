# Memory model and ownership

Kove's long-term goal is compile-time memory safety:

- no use-after-free
- no double-free
- no invalid memory access caused by ownership violations
- no data races in safe Kove code
- explicit `unsafe` where the guarantees must be waived

The ownership model will be designed deliberately and documented in this
file before any of it is implemented. It won't be copied blindly from
another language, and it won't grow by accident. So this document has
two parts: the model the language actually has today, and the design
space for the real one.

## The model today (v0.1): value semantics

Every Kove value today behaves like a self-contained value:

- `let b = a;`, argument passing, and `return` all copy. Mutating a
  copy never affects the original. This is observable and tested
  (`tests/tests/compiler.rs::value_semantics_copy_on_assignment`).
- There are no references, no pointers, and no way to alias a value.
  `&data` and `&mut data` are future syntax and don't parse.
- Deallocation is trivially safe: the interpreter's values die with
  their scope.

This is deliberately the simplest model that is sound. Nothing in it can
dangle, race, or double-free, so no program written today can be
invalidated by the future model. The ownership system can only make more
programs expressible (cheap borrowing instead of copying), not break
existing ones.

The cost is performance (struct copies). That's acceptable for a
tree-walking interpreter, and it's exactly the pressure that will
motivate references.

## Design space for the real model

Questions the design must answer before implementation starts, with the
current leanings:

1. Move or copy by default? Today everything copies. Once values get
   heap-backed contents (collections, big structs), copying everything
   stops being honest about cost. Likely direction: move-by-default for
   heap-owning types with explicit `.clone()`, copy for the small
   primitives. "Always copy, optimize internally" stays on the table
   for its simplicity.
2. Borrowing. `&data` (shared, read-only) and `&mut data` (exclusive)
   as second-class borrows first: allowed in parameters and locals, not
   storable in structs. That covers most of the performance need
   without lifetime annotations. Storable references and named
   lifetimes only if real programs demand them.
3. The aliasing rule. The likely invariant is shared-xor-mutable: any
   number of `&`, or exactly one `&mut`. It's what makes the no-race
   guarantee provable when threads arrive.
4. Escape analysis vs annotations. Prefer rules a user can predict from
   the code they see ("a borrow may not outlive the block that created
   it") over inference cleverness. Clarity and predictability are core
   Kove values.
5. `unsafe`. A keyword-scoped block whose operations are enumerated and
   documented. Safe Kove keeps all guarantees.

Non-goals for the first ownership iteration: garbage collection,
reference counting as the model (it may appear as a library type), and
lifetime syntax in surface Kove.

## Process

The model ships in this order, each step landing in this document first:

1. Written semantics (this file) with examples of accepted and rejected
   programs.
2. Borrow rules enforced in the type checker, behind tests, while the
   interpreter still copies. The checker gets stricter, the runtime
   stays the same, and programs that pass keep their behavior.
3. Only then: runtime representation changes (in-place moves, borrow
   pointers) in the IR and backend phases.
