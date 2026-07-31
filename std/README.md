# std

**Status:** not implemented. Lands in v0.6/v0.7, after modules exist.

Kove's standard library, written in Kove.

## What exists today

`println` only, and it is a builtin rather than a library function: the
compiler knows its name, and the type checker special-cases it. That is
the honest state of things, and it is why this directory has no `.kov`
files yet. A standard library needs modules to be importable, and
`import` currently reports E0217.

## Planned modules

The requirements are explicit that std stays small. First:

```text
std::io          input and output
std::fs          files
std::string      string operations beyond the built-in type
std::collections lists and maps
std::process     running and exiting
std::path        path manipulation
```

Later, once the language can express them well: `std::net`,
`std::sync`, `std::thread`, `std::time`, `std::crypto`.

## Constraints already decided

- Written in Kove, not Rust, except for the irreducible primitives that
  need to talk to the operating system. A standard library the language
  cannot express is a sign the language is missing something.
- Small on purpose. Every module here is a compatibility promise.
- `println` becomes `std::io` when modules land, and the builtin goes
  away. That is a breaking change and gets called out in the changelog
  when it happens.
