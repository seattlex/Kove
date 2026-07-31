# lsp

**Status:** not implemented. Lands in v0.7.

`kove-lsp`, a language server speaking LSP over stdio.

## Why this is closer than it looks

The requirement is that the language server reuse the compiler frontend
rather than implementing a second parser, and the frontend was built for
that from the first commit:

- **Incremental reparsing.** `Document::edit` reparses only what an edit
  could reach. Keystroke-latency editing is the engine's job, already
  done.
- **Error recovery.** Half-typed code still produces a complete tree, so
  every feature keeps working while the user is mid-edit.
- **Editor annotations on the grammar.** Highlighting rules, document
  symbols and folding ranges are declared in `compiler/parser`
  alongside the rules they describe, so they cannot drift from the
  grammar.
- **Spans everywhere.** Every diagnostic and every AST node carries byte
  ranges, and the resolver maps references to the bindings they name,
  which is go-to-definition and find-references.

ReParse ships `reparse-lsp`, a working stdio server over the same
engine, so the protocol layer is a solved problem to borrow from rather
than invent.

## Capabilities, in the order they should land

| Capability | Comes from |
| --- | --- |
| publishDiagnostics | the driver, unchanged |
| incremental sync | `Document::edit` |
| semantic tokens | grammar highlight rules |
| document symbols | grammar `symbol()` annotations |
| folding ranges | grammar `foldable()` annotations |
| go-to-definition | resolver: reference id to binding |
| find references | resolver: the same map, inverted |
| hover | type checker: the type of the expression under the cursor |
| rename | resolver, once definition and references are exact |
| formatting | the formatter crate |

## Constraints already decided

- No second parser, no second type checker, no second set of
  diagnostics. If the server disagrees with `kove check`, that is a bug.
- The server exposes what the compiler already computes. A feature that
  needs new analysis is a compiler change first.
