# compiler/mir

**Status:** not implemented. Lands in v0.6, after HIR.

MIR (mid-level intermediate representation) is Kove's control-flow
graph: the form where "what runs in what order" is explicit and
structured statements are gone.

## Shape

Per function: a list of basic blocks, each a straight-line sequence of
statements ending in exactly one terminator (goto, branch, return,
call). Locals are numbered slots. Nothing nests.

```text
bb0: _1 = const 0
     goto -> bb1
bb1: _2 = Lt(_1, const 10)
     switchInt(_2) -> [true: bb2, false: bb3]
bb2: ...
```

## Why it exists

- Code generation against a CFG is straightforward. Against a tree it
  is a pile of special cases.
- The analyses Kove has committed to want a CFG. "Does every path
  return" is answered conservatively on the AST today (see
  `always_returns` in the type checker); on a CFG it is exact. The
  ownership and borrow rules in `docs/ownership.md` need dataflow, and
  dataflow needs blocks.
- Optimization, eventually. Not before correctness, per the engineering
  principles, but the structure should not have to be rebuilt to allow
  it.

## Constraints already decided

- Every MIR statement keeps the span of the HIR it came from. A runtime
  error in optimized code still points at source.
- Checked arithmetic is explicit in MIR, not implied. Kove's Int
  arithmetic traps on overflow and division by zero, so those checks are
  real operations here, which is also what lets a later pass remove the
  ones it can prove redundant.
