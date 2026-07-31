//! The registry of diagnostic codes.
//!
//! Every code the compiler can emit has an entry here with a longer
//! explanation than fits on the diagnostic itself. `kove explain E0012`
//! prints one.
//!
//! This is the single source of truth for what codes exist. A test checks
//! it against the table in `docs/diagnostics.md`, so the documentation and
//! the compiler cannot drift apart.

/// One code, its one-line summary, and the longer explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    pub code: &'static str,
    pub summary: &'static str,
    pub explanation: &'static str,
}

/// Look a code up. Case-insensitive, so `e0012` works as well as `E0012`.
pub fn explain(code: &str) -> Option<&'static CodeInfo> {
    CODES.iter().find(|c| c.code.eq_ignore_ascii_case(code))
}

pub const CODES: &[CodeInfo] = &[
    // --- Lexical -----------------------------------------------------
    CodeInfo {
        code: "E0001",
        summary: "unrecognized character",
        explanation: "\
A character in the source is not part of any Kove token.

This is usually a stray symbol, a smart quote pasted from a document, or
a character from another language's syntax. Kove's punctuation is listed
in docs/syntax.md.",
    },
    CodeInfo {
        code: "E0101",
        summary: "expected a specific token",
        explanation: "\
Something the grammar requires is missing, most often a `;`, `)`, `}` or
`,`.

The parser inserts the missing token and carries on, so one typo
produces one error rather than a cascade. If several of these appear at
once, fix the first and re-check: the later ones are often consequences
of it.",
    },
    CodeInfo {
        code: "E0102",
        summary: "expected an expression",
        explanation: "\
An operator or keyword was followed by nothing usable as a value.

A dangling `+` at the end of a line and an empty `return` in a function
that must return something both land here.",
    },
    CodeInfo {
        code: "E0103",
        summary: "unexpected input",
        explanation: "\
A stretch of source matched no rule, so the parser skipped it to reach
code it could understand again.

The skipped range is what the carets cover. This usually means a
construct is malformed rather than merely incomplete.",
    },
    CodeInfo {
        code: "E0104",
        summary: "statement at the top level",
        explanation: "\
Only declarations live at the top level of a file: functions, structs,
enums and imports.

Move the statement into a function. If it is the program's work, that
function is `fn main()`.",
    },
    CodeInfo {
        code: "E0110",
        summary: "integer literal is too large",
        explanation: "\
`Int` is a signed 64-bit integer, so the largest literal is
9223372036854775807.

Sized integer types are reserved for later; for now, a value beyond this
range cannot be written as a literal.",
    },
    CodeInfo {
        code: "E0111",
        summary: "unknown escape sequence",
        explanation: "\
A backslash in a string or character literal was followed by something
Kove does not define.

The supported escapes are \\n, \\t, \\r, \\0, \\\\, \\\" and \\'. To write a
literal backslash, double it.",
    },
    CodeInfo {
        code: "E0112",
        summary: "unterminated string literal",
        explanation: "\
A string opened with `\"` and the line ended before it closed.

Strings do not span lines. A closing quote is missing, or a quote inside
the text needs escaping as \\\".",
    },
    CodeInfo {
        code: "E0113",
        summary: "unterminated character literal",
        explanation: "\
A character literal opened with `'` and never closed.

A character literal holds exactly one character: `'k'`, or an escape
such as `'\\n'`. For text, use a string in double quotes.",
    },
    CodeInfo {
        code: "E0114",
        summary: "unterminated block comment",
        explanation: "\
A `/*` was never closed with `*/`.

Block comments do not nest, so a `/*` inside one does not open a second
comment and the first `*/` closes it. An unclosed comment is an error
rather than a silent comment to the end of the file, because that
silently deletes code.",
    },
    // --- Types and names ---------------------------------------------
    CodeInfo {
        code: "E0012",
        summary: "mismatched types",
        explanation: "\
An expression has one type where another was required.

Kove never converts between types implicitly, not even from Int to
Float, so both sides have to agree exactly. Change the value, or change
the annotation, or convert explicitly.

This code covers every place two types must line up: a `let` and its
annotation, an argument and its parameter, a `return` and the declared
return type, an assignment and the variable's type, and the bounds of a
range.",
    },
    CodeInfo {
        code: "E0200",
        summary: "cannot find a type",
        explanation: "\
A type name does not refer to anything.

The primitive types are Int, Float, Bool, Char and String. Any other
type has to be declared as a struct or an enum in the same file, since
there are no modules yet. Where a close name exists, the diagnostic
suggests it.",
    },
    CodeInfo {
        code: "E0201",
        summary: "cannot find a variable",
        explanation: "\
A name is not bound to anything at this point in the program.

A variable exists from its `let` to the end of the enclosing block, so
this often means the declaration is in an inner block, or comes later in
the file. If the name is a function, it needs to be called: `name(...)`.",
    },
    CodeInfo {
        code: "E0202",
        summary: "cannot find a function",
        explanation: "\
A call names a function that is not declared anywhere in the file.

Functions can be declared in any order, so this is a spelling mistake or
a missing declaration rather than an ordering problem. If the name is a
variable, it cannot be called: Kove has no function values yet.",
    },
    CodeInfo {
        code: "E0203",
        summary: "wrong number of arguments",
        explanation: "\
A call passes a different number of arguments than the function takes.

Kove has no optional or variadic parameters, so the counts have to match
exactly. The note on the diagnostic shows the signature.",
    },
    CodeInfo {
        code: "E0204",
        summary: "assignment to something immutable",
        explanation: "\
Variables are immutable by default, and this one was not declared with
`mut`.

Write `let mut name = ...` to allow assignment. Function parameters and
`for` loop variables are always immutable; to change one, copy it into a
`let mut` first.",
    },
    CodeInfo {
        code: "E0205",
        summary: "duplicate definition",
        explanation: "\
A name is defined twice in a place that allows only one definition.

This covers functions, structs, enums, struct fields, enum variants and
parameters, and also the built-in names `println` and `assert`, which
cannot be redefined. Shadowing a *variable* with a later `let` is
allowed and is not this error.",
    },
    CodeInfo {
        code: "E0206",
        summary: "no such field",
        explanation: "\
A struct does not have the field being read or set.

The diagnostic lists the fields it does have. Field names are
case-sensitive.",
    },
    CodeInfo {
        code: "E0207",
        summary: "missing fields in a struct literal",
        explanation: "\
A struct literal has to give every field a value.

There are no default values and no partial initialization, so a struct
value is always complete. The diagnostic names the fields left out.",
    },
    CodeInfo {
        code: "E0208",
        summary: "duplicate field in a struct literal",
        explanation: "\
A struct literal gives the same field a value twice.

Only one of the two would take effect, so rather than pick silently,
Kove rejects it.",
    },
    CodeInfo {
        code: "E0209",
        summary: "field access on a value with no fields",
        explanation: "\
Only structs have fields.

Reading `.name` on an Int, a String or an enum value is not defined.
Methods do not exist yet, so `.` is always field access.",
    },
    CodeInfo {
        code: "E0210",
        summary: "not all paths return a value",
        explanation: "\
A function declares a return type but has a way to reach its end without
returning.

The check is deliberately conservative: a path counts as returning if it
ends in `return`, or in an if/else chain where every branch returns. A
loop body never counts, because it may run zero times. Adding a `return`
at the end of the function is always enough.",
    },
    CodeInfo {
        code: "E0211",
        summary: "condition is not Bool",
        explanation: "\
An `if` or `while` condition, or an argument to `assert`, has to be a
`Bool`.

Kove has no truthiness: a number is not a condition and neither is a
string. Write the comparison you mean, such as `x != 0`.",
    },
    CodeInfo {
        code: "E0212",
        summary: "operator does not apply to these types",
        explanation: "\
An operator was used on operand types it is not defined for.

Arithmetic and ordering need two Ints or two Floats, never one of each,
because Kove does not convert implicitly. Logical operators need Bools.
Equality needs two values of the same comparable type; structs are not
comparable yet.",
    },
    CodeInfo {
        code: "E0213",
        summary: "invalid assignment target",
        explanation: "\
The left side of an assignment has to be a place: a variable, or a chain
of fields rooted at one.

A call result, a literal or an arithmetic expression cannot be assigned
to, because there is nothing to assign into.",
    },
    CodeInfo {
        code: "E0214",
        summary: "missing or malformed main",
        explanation: "\
`kove run` and `kove build` need an entry point: `fn main()` with no
parameters and no return type.

`kove check` does not require one, so a file of functions still
type-checks. `kove test` does not require one either.",
    },
    CodeInfo {
        code: "E0215",
        summary: "value cannot be printed",
        explanation: "\
`println` accepts Int, Float, Bool, Char and String.

Printing a struct or an enum value is not supported yet. Print the
fields you care about instead.",
    },
    CodeInfo {
        code: "E0216",
        summary: "no such enum variant",
        explanation: "\
An `Enum::Variant` path names a variant the enum does not have, or names
something that is not an enum at all.

The diagnostic lists the variants that do exist. To build a struct, use
a struct literal rather than a path.",
    },
    CodeInfo {
        code: "E0217",
        summary: "modules are not implemented yet",
        explanation: "\
`import` parses, but there is no module system behind it.

A Kove program is currently a single file. The keyword is reserved and
reported rather than ignored, so the single-file model is never mistaken
for the module system that is planned.",
    },
    CodeInfo {
        code: "E0218",
        summary: "for loop needs a range",
        explanation: "\
`for` iterates over an Int range, written `lo..hi`.

Ranges are half-open, so `0..3` visits 0, 1 and 2, and a reversed or
empty range runs zero times. Iterating over collections is planned but
does not exist yet.",
    },
    CodeInfo {
        code: "E0219",
        summary: "struct literal syntax on an enum",
        explanation: "\
Enums are constructed by naming a variant, not with braces.

Write `Status::Active`. Variants carrying data are planned, but today
every variant is a plain value.",
    },
    CodeInfo {
        code: "E0220",
        summary: "test function cannot be run",
        explanation: "\
A function whose name begins with `test_` is a test, and `kove test`
calls it with no arguments and ignores no result.

So a test has to take no parameters and return nothing. Skipping such a
function silently would be worse than saying so, since it looks like a
test that passes.",
    },
    CodeInfo {
        code: "E0230",
        summary: "only named functions can be called",
        explanation: "\
The thing before `(` has to be the name of a declared function or a
built-in.

Methods, function values and calling the result of an expression are all
outside the language today.",
    },
    // --- Warnings ----------------------------------------------------
    CodeInfo {
        code: "W0001",
        summary: "binding is never read",
        explanation: "\
A `let`, a parameter or a `for` variable holds a value nothing ever
reads.

Assigning to a variable does not count as reading it, so one that is
only ever written still warns. Rename it with a leading underscore
(`_name`) to say the binding is deliberate, or remove it.",
    },
    CodeInfo {
        code: "W0002",
        summary: "function is never called",
        explanation: "\
No execution starting at `main` or at a test can reach this function.

The test is reachability, not whether the name appears in a call: a
function that only calls itself, or a pair that only call each other, is
still dead. A leading underscore exempts a function, and a file with no
entry point at all is left alone entirely.",
    },
    // --- Runtime ------------------------------------------------------
    CodeInfo {
        code: "E0301",
        summary: "division by zero",
        explanation: "\
An Int division had a zero divisor.

Int arithmetic is checked rather than undefined, so this stops the
program with a diagnostic instead of producing a wrong number. Float
division follows IEEE 754 and gives infinity instead.",
    },
    CodeInfo {
        code: "E0302",
        summary: "integer overflow",
        explanation: "\
An Int operation produced a value outside the 64-bit signed range.

Kove does not wrap silently: a result that does not fit is an error, at
the operation that produced it. Float arithmetic follows IEEE 754 and
does not raise this.",
    },
    CodeInfo {
        code: "E0303",
        summary: "remainder by zero",
        explanation: "\
An Int remainder (`%`) had a zero divisor, which is undefined for the
same reason division is.",
    },
    CodeInfo {
        code: "E0304",
        summary: "recursion limit reached",
        explanation: "\
Nested Kove calls went deeper than the interpreter allows.

This is almost always recursion with a base case that is never reached.
The limit exists so runaway recursion produces a diagnostic pointing at
the call rather than crashing the process.",
    },
    CodeInfo {
        code: "E0305",
        summary: "failed to write output",
        explanation: "\
`println` could not write to the output stream.

This is an environment problem rather than a problem with the program:
a closed pipe or a full disk.",
    },
    CodeInfo {
        code: "E0306",
        summary: "assertion failed",
        explanation: "\
An `assert` condition evaluated to false.

The span points at the condition rather than the call, so the caret
shows what did not hold. Under `kove test` this is how a test fails.",
    },
];
