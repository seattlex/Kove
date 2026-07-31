# Kove syntax reference

The grammar as implemented in `compiler/lexer` (tokens) and
`compiler/parser` (rules), which together are the single source of
truth; this document mirrors them. Both are defined declaratively on the
[ReParse](https://github.com/seattlex/ReParse) engine, which gives every
construct error recovery: any input, no matter how broken, parses to a
complete tree with explicit `missing` tokens and error islands.

## Lexical structure

Tokens are matched longest-first, so `1.5` is one float token while
`1..10` is `1`, `..`, `10`, and `intx` is an identifier, not a keyword.

| Token | Form |
| --- | --- |
| identifier | `[A-Za-z_][A-Za-z0-9_]*` |
| int | `[0-9]+` |
| float | `[0-9]+.[0-9]+` |
| string | `"..."` with escapes `\n \t \r \0 \\ \" \'` |
| char | `'x'` or `'\n'` (one character) |
| line comment | `// to end of line` |
| block comment | `/* ... */` (non-nesting, must be closed) |

Keywords (reserved, never identifiers):

```text
fn let mut return if else while for in struct enum import true false match
```

Operators and punctuation:

```text
( ) { } , ; : :: . .. -> = == != < <= > >= + - * / % ! && ||
```

## Grammar

EBNF-style; `*` is repetition, `?` is optional, `|` is choice.
Comments and whitespace can appear between any two tokens.

```ebnf
source_file   = item* ;
item          = function | struct_decl | enum_decl | import_decl ;

function      = "fn" identifier "(" parameters? ")" ( "->" type )? block ;
parameters    = parameter ( "," parameter )* ","? ;
parameter     = identifier ":" type ;

struct_decl   = "struct" identifier "{" field_decls? "}" ;
field_decls   = field_decl ( "," field_decl )* ","? ;
field_decl    = identifier ":" type ;

enum_decl     = "enum" identifier "{" variants? "}" ;
variants      = identifier ( "," identifier )* ","? ;

import_decl   = "import" identifier ( "::" identifier )* ";" ;   (* E0217 *)

type          = identifier ;

block         = "{" statement* "}" ;
statement     = let_stmt | return_stmt | if_stmt | while_stmt
              | for_stmt | block | assign_stmt | expr_stmt ;

let_stmt      = "let" "mut"? identifier ( ":" type )? "=" expression ";" ;
assign_stmt   = postfix_expr "=" expression ";" ;
return_stmt   = "return" expression? ";" ;
if_stmt       = "if" expression block ( "else" ( if_stmt | block ) )? ;
while_stmt    = "while" expression block ;
for_stmt      = "for" identifier "in" expression block ;
expr_stmt     = expression ";" ;

expression    = range ;
range         = or  ( ".." or )* ;
or            = and ( "||" and )* ;
and           = eq  ( "&&" eq )* ;
eq            = cmp ( ("==" | "!=") cmp )* ;
cmp           = add ( ("<" | "<=" | ">" | ">=") add )* ;
add           = mul ( ("+" | "-") mul )* ;
mul           = unary ( ("*" | "/" | "%") unary )* ;
unary         = ("!" | "-") unary | postfix_expr ;
postfix_expr  = primary ( call_suffix | field_suffix )* ;
call_suffix   = "(" ( expression ( "," expression )* )? ")" ;
field_suffix  = "." identifier ;

primary       = int | float | string | char | "true" | "false"
              | path_expr | struct_literal | identifier
              | "(" expression ")" ;
path_expr     = identifier "::" identifier ;
struct_literal= identifier "{" field_init ( "," field_init )* ","? "}" ;
field_init    = identifier ":" expression ;
```

All binary operators associate left. Trailing commas are allowed in
parameter lists, struct/enum declarations and struct literals.

### The struct-literal / block interaction

In `if ready { }`, is `ready { }` a struct literal? Kove settles this
without a special-cased restriction: a struct literal requires at least
one field, so `ready { }` can't be one, and a block starting with any
real statement never looks like a field list (`name: value`). The
parser tries the literal interpretation, fails, and falls back to
condition-then-block. The consequence is that empty struct literals are
not part of the grammar.

## Error recovery

The parser guarantees a complete tree for every input:

- A missing `)`, `}`, `,` or `;` is inserted as a zero-width token and
  reported (E0101). Parsing continues, so one typo yields one error.
- Unparseable stretches become error islands (E0103) bounded by the
  enclosing construct's closers, and the rest of the file parses
  normally.
- Unterminated strings, chars and block comments have bounded extents
  (to the end of the line or file) and their own codes (E0112 to E0114)
  instead of cascading lex errors.

This recovery is the same machinery an editor needs, which is why the
frontend is already suitable for the future language server.
