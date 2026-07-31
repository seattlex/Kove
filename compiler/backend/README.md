# compiler/backend

**Status:** not implemented. This is v0.6, and the largest single gap
between Kove today and Kove as a systems language.

Turns MIR into a native executable. First target: **x86-64 Linux**.
Other targets come after one works end to end.

## The decision to make first

Three routes, and the choice belongs in this file before any code:

| Route | For | Against |
| --- | --- | --- |
| **Cranelift** | Rust-native, fast to compile, no external toolchain, good debug-build codegen | Weaker optimized output than LLVM, smaller target list |
| **LLVM** | Best codegen, every target, mature | Heavy dependency, slow builds, big surface to bind against, awkward for a self-hosting story |
| **Direct x86-64 emission** | No dependencies at all, total control, teaches the most | Every target from scratch, register allocation is real work, slowest path to "it works" |

Current lean: **Cranelift first**, direct emission later if the
self-hosting goal makes an external dependency uncomfortable. Cranelift
gets a working native compiler soonest, which is what the roadmap needs;
the MIR boundary means the backend can be replaced without touching any
earlier stage.

## Constraints already decided

- `kove build` produces a native executable and `kove run` keeps
  working. Until then `build` says plainly that codegen does not exist
  rather than pretending.
- The interpreter stays as the reference implementation. When both
  exist, the same test programs run through both and must agree, which
  is what makes the interpreter worth keeping.
- Runtime checks are not dropped in the name of speed. Int overflow and
  division by zero abort with a diagnostic in compiled code too.
