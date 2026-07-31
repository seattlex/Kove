//! The Kove language frontend: the grammar definition on the ReParse
//! engine, plus the mapping from ReParse's recovery diagnostics to Kove
//! error codes.
//!
//! ReParse covers the Lexer -> Parser -> Concrete Syntax Tree stages of the
//! pipeline and gives us incremental reparsing, error recovery (any input
//! produces a complete tree), syntax highlighting, and LSP-ready trees.
//! The same frontend will back `kove-lsp` later, so the compiler and the
//! language server can never disagree about syntax.
//!
//! Everything downstream (AST lowering, type checking) consumes the tree
//! through node kind names ("function_declaration", "let_statement", ...)
//! and field names ("name", "value", ...) declared here.

use kove_diagnostics::{Diagnostic, Span};
use reparse::grammar::*;
use reparse::green::DiagMessage;
use reparse::highlight::HighlightClass as Hc;
use reparse::{Document, SyntaxElem};
use std::sync::{Arc, OnceLock};

pub use reparse;

/// The Kove language definition. Built once, shared everywhere.
pub fn language() -> Arc<Language> {
    static LANG: OnceLock<Arc<Language>> = OnceLock::new();
    LANG.get_or_init(build_language).clone()
}

fn build_language() -> Arc<Language> {
    let mut g = GrammarBuilder::new("kove");

    // --- Trivia -----------------------------------------------------------
    g.trivia("whitespace", r"[ \t\r\n]+").unwrap();
    let line_comment = g.trivia("line_comment", r"//[^\n]*").unwrap();
    // The pattern includes the unterminated form so an open `/*` has a
    // defined extent; `syntax_diagnostics` reports it as E0114.
    let block_comment = g
        .trivia("block_comment", r"/\*([^*]|\*(?!/))*(\*/)?")
        .unwrap();

    // --- Tokens -----------------------------------------------------------
    // The lexer picks the longest match, so `1.5` is one float, `1..10` is
    // int, `..`, int, and `intx` is an identifier rather than a keyword.
    let ident = g.token("identifier", r"[A-Za-z_]\w*").unwrap();
    let float = g.token("float", r"\d+\.\d+").unwrap();
    let int = g.token("int", r"\d+").unwrap();
    let string = g.token("string", r#""([^"\\\n]|\\.)*""#).unwrap();
    let string_open = g
        .token("unterminated_string", r#""([^"\\\n]|\\.)*\\?"#)
        .unwrap();
    let char_lit = g.token("char", r"'([^'\\\n]|\\.)'").unwrap();
    let char_open = g.token("unterminated_char", r"'([^'\\\n]|\\.)?").unwrap();

    let lparen = g.punct("(");
    let rparen = g.punct(")");
    let lbrace = g.punct("{");
    let rbrace = g.punct("}");
    let comma = g.punct(",");
    let semi = g.punct(";");
    let colon = g.punct(":");
    let coloncolon = g.punct("::");
    let dot = g.punct(".");
    let dotdot = g.punct("..");
    let arrow = g.punct("->");
    let eq = g.punct("=");
    let (eqeq, neq) = (g.punct("=="), g.punct("!="));
    let (le, ge, lt, gt) = (g.punct("<="), g.punct(">="), g.punct("<"), g.punct(">"));
    let (plus, minus, star, slash, percent) = (
        g.punct("+"),
        g.punct("-"),
        g.punct("*"),
        g.punct("/"),
        g.punct("%"),
    );
    let (andand, oror, bang) = (g.punct("&&"), g.punct("||"), g.punct("!"));

    let kw_fn = g.keyword(ident, "fn");
    let kw_let = g.keyword(ident, "let");
    let kw_mut = g.keyword(ident, "mut");
    let kw_return = g.keyword(ident, "return");
    let kw_if = g.keyword(ident, "if");
    let kw_else = g.keyword(ident, "else");
    let kw_while = g.keyword(ident, "while");
    let kw_for = g.keyword(ident, "for");
    let kw_in = g.keyword(ident, "in");
    let kw_struct = g.keyword(ident, "struct");
    let kw_enum = g.keyword(ident, "enum");
    let kw_import = g.keyword(ident, "import");
    let kw_true = g.keyword(ident, "true");
    let kw_false = g.keyword(ident, "false");
    // Reserved for pattern matching (docs/language.md); has no grammar yet.
    let kw_match = g.keyword(ident, "match");

    // Tokens recovery may insert as zero-width "missing" leaves.
    g.recover_tokens(&[rparen, rbrace, comma, semi]);

    // --- Expressions ------------------------------------------------------
    let expression = g.declare("expression");
    g.make_inline(expression);
    let unary = g.declare("unary");
    g.make_inline(unary);
    let postfix_expr = g.declare("postfix_expr");
    g.make_inline(postfix_expr);
    let block = g.declare("block");
    let if_stmt = g.declare("if_statement");
    let type_expr = g.rule("type_expression", field("name", t(ident)));

    let arguments = g.rule(
        "arguments",
        seq(vec![
            t(lparen),
            rep0(field("arg", r(expression)))
                .sep(t(comma))
                .recover(&[rparen, semi, rbrace]),
            t(rparen),
        ]),
    );

    let paren_expr = g.rule(
        "paren_expression",
        seq(vec![t(lparen), field("inner", r(expression)), t(rparen)]),
    );

    // `Enum::Variant`
    let path_expr = g.rule(
        "path_expression",
        seq(vec![
            field("base", t(ident)),
            t(coloncolon),
            field("name", t(ident)),
        ]),
    );

    // `User { name: "Alex", age: 20 }`. Requires at least one field and has
    // no recovery point on purpose: together those guarantee that a block
    // after a bare-identifier condition (`if ready { ... }`) can never be
    // captured as a struct literal: the literal alternative fails and the
    // parser backtracks to the plain identifier.
    let field_init = g.rule(
        "field_initializer",
        seq(vec![
            field("name", t(ident)),
            t(colon),
            field("value", r(expression)),
        ]),
    );
    let struct_lit = g.rule(
        "struct_literal",
        seq(vec![
            field("name", t(ident)),
            t(lbrace),
            rep1(r(field_init)).sep(t(comma)).allow_trailing(),
            t(rbrace),
        ]),
    );

    let primary = g.inline(
        "primary",
        choice(vec![
            t(float),
            t(int),
            t(string),
            t(string_open),
            t(char_lit),
            t(char_open),
            t(kw_true),
            t(kw_false),
            r(path_expr),
            r(struct_lit),
            t(ident),
            r(paren_expr),
        ]),
    );

    let unary_expr = g.rule(
        "unary_expression",
        seq(vec![
            field("op", choice(vec![t(bang), t(minus)])),
            field("operand", r(unary)),
        ]),
    );

    g.define(
        postfix_expr,
        postfix(
            r(primary),
            vec![
                suffix("call_expression", r(arguments)).base_field("callee"),
                suffix(
                    "field_expression",
                    seq(vec![t(dot), field("name", t(ident))]),
                )
                .base_field("base"),
            ],
        ),
    );

    g.define(unary, choice(vec![r(unary_expr), r(postfix_expr)]));

    // levels[0] binds loosest.
    g.define(
        expression,
        binop(
            r(unary),
            vec![
                level(&[dotdot], Assoc::Left, "range_expression").fields("lhs", "op", "rhs"),
                level(&[oror], Assoc::Left, "binary_expression").fields("lhs", "op", "rhs"),
                level(&[andand], Assoc::Left, "binary_expression").fields("lhs", "op", "rhs"),
                level(&[eqeq, neq], Assoc::Left, "binary_expression").fields("lhs", "op", "rhs"),
                level(&[lt, gt, le, ge], Assoc::Left, "binary_expression")
                    .fields("lhs", "op", "rhs"),
                level(&[plus, minus], Assoc::Left, "binary_expression").fields("lhs", "op", "rhs"),
                level(&[star, slash, percent], Assoc::Left, "binary_expression")
                    .fields("lhs", "op", "rhs"),
            ],
        ),
    );

    // --- Statements -------------------------------------------------------
    let let_stmt = g.rule(
        "let_statement",
        seq(vec![
            t(kw_let),
            opt(field("mutable", t(kw_mut))),
            field("name", t(ident)),
            opt(seq(vec![t(colon), field("type", r(type_expr))])),
            t(eq),
            field("value", r(expression)),
            t(semi),
        ]),
    );

    let assign_stmt = g.rule(
        "assignment_statement",
        seq(vec![
            field("target", r(postfix_expr)),
            t(eq),
            field("value", r(expression)),
            t(semi),
        ]),
    );

    let return_stmt = g.rule(
        "return_statement",
        seq(vec![
            t(kw_return),
            opt(field("value", r(expression))),
            t(semi),
        ]),
    );

    let while_stmt = g.rule(
        "while_statement",
        seq(vec![
            t(kw_while),
            field("condition", r(expression)),
            field("body", r(block)),
        ]),
    );

    let for_stmt = g.rule(
        "for_statement",
        seq(vec![
            t(kw_for),
            field("name", t(ident)),
            t(kw_in),
            field("iter", r(expression)),
            field("body", r(block)),
        ]),
    );

    let expr_stmt = g.rule(
        "expression_statement",
        seq(vec![field("value", r(expression)), t(semi)]),
    );

    let statement = g.inline(
        "statement",
        choice(vec![
            r(let_stmt),
            r(return_stmt),
            r(if_stmt),
            r(while_stmt),
            r(for_stmt),
            r(block),
            r(assign_stmt),
            r(expr_stmt),
        ]),
    );

    g.define(
        block,
        seq(vec![
            t(lbrace),
            rep0(r(statement)).recover(&[rbrace]),
            t(rbrace),
        ]),
    );

    g.define(
        if_stmt,
        seq(vec![
            t(kw_if),
            field("condition", r(expression)),
            field("then", r(block)),
            opt(seq(vec![
                t(kw_else),
                field("else", choice(vec![r(if_stmt), r(block)])),
            ])),
        ]),
    );

    // --- Items ------------------------------------------------------------
    let parameter = g.rule(
        "parameter",
        seq(vec![
            field("name", t(ident)),
            t(colon),
            field("type", r(type_expr)),
        ]),
    );
    let parameter_list = g.rule(
        "parameter_list",
        seq(vec![
            t(lparen),
            rep0(r(parameter))
                .sep(t(comma))
                .allow_trailing()
                .recover(&[rparen, lbrace, rbrace, semi]),
            t(rparen),
        ]),
    );

    let function = g.rule(
        "function_declaration",
        seq(vec![
            t(kw_fn),
            field("name", t(ident)),
            field("params", r(parameter_list)),
            opt(seq(vec![t(arrow), field("return_type", r(type_expr))])),
            field("body", r(block)),
        ]),
    );

    let field_decl = g.rule(
        "field_declaration",
        seq(vec![
            field("name", t(ident)),
            t(colon),
            field("type", r(type_expr)),
        ]),
    );
    let struct_decl = g.rule(
        "struct_declaration",
        seq(vec![
            t(kw_struct),
            field("name", t(ident)),
            t(lbrace),
            rep0(r(field_decl))
                .sep(t(comma))
                .allow_trailing()
                .recover(&[rbrace]),
            t(rbrace),
        ]),
    );

    let variant = g.rule("variant", field("name", t(ident)));
    let enum_decl = g.rule(
        "enum_declaration",
        seq(vec![
            t(kw_enum),
            field("name", t(ident)),
            t(lbrace),
            rep0(r(variant))
                .sep(t(comma))
                .allow_trailing()
                .recover(&[rbrace]),
            t(rbrace),
        ]),
    );

    let import_path = g.rule(
        "import_path",
        seq(vec![
            t(ident),
            rep0(seq(vec![t(coloncolon), t(ident)])),
        ]),
    );
    let import_decl = g.rule(
        "import_declaration",
        seq(vec![t(kw_import), field("path", r(import_path)), t(semi)]),
    );

    // Statements are included at the top level so a stray `let` outside a
    // function gets a targeted E0104 from lowering instead of a generic
    // recovery error.
    let item = g.inline(
        "item",
        choice(vec![
            r(function),
            r(struct_decl),
            r(enum_decl),
            r(import_decl),
            r(statement),
        ]),
    );
    let source_file = g.rule("source_file", rep0(r(item)).recover(&[]));
    g.root(source_file);

    // --- Editor annotations -----------------------------------------------
    g.symbol(function, "name", SymbolKind::Function);
    g.symbol(struct_decl, "name", SymbolKind::Struct);
    g.symbol(enum_decl, "name", SymbolKind::Enum);
    g.symbol(field_decl, "name", SymbolKind::Field);
    g.symbol(let_stmt, "name", SymbolKind::Variable);
    g.foldable(&[block, parameter_list, arguments, struct_decl, enum_decl]);

    for kw in [
        kw_fn, kw_let, kw_mut, kw_return, kw_if, kw_else, kw_while, kw_for, kw_in, kw_struct,
        kw_enum, kw_import, kw_match,
    ] {
        g.highlight(hl(Hc::Keyword).token(kw));
    }
    for c in [kw_true, kw_false] {
        g.highlight(hl(Hc::Constant).token(c));
    }
    // First matching rule wins.
    g.highlight(hl(Hc::Type).token(ident).in_rule(type_expr));
    g.highlight(hl(Hc::Type).token(ident).field("name").in_rule(struct_decl));
    g.highlight(hl(Hc::Type).token(ident).field("name").in_rule(enum_decl));
    g.highlight(hl(Hc::Type).token(ident).field("name").in_rule(struct_lit));
    g.highlight(hl(Hc::Type).token(ident).field("base").in_rule(path_expr));
    g.highlight(hl(Hc::Function).token(ident).field("name").in_rule(function));
    g.highlight(hl(Hc::Function).token(ident).field("callee"));
    g.highlight(hl(Hc::Parameter).token(ident).in_rule(parameter));
    g.highlight(
        hl(Hc::Property)
            .token(ident)
            .field("name")
            .in_node("field_expression"),
    );
    g.highlight(hl(Hc::Property).token(ident).in_rule(field_decl));
    g.highlight(hl(Hc::Property).token(ident).in_rule(field_init));
    g.highlight(hl(Hc::Number).token(int));
    g.highlight(hl(Hc::Number).token(float));
    g.highlight(hl(Hc::String).token(string));
    g.highlight(hl(Hc::String).token(string_open));
    g.highlight(hl(Hc::String).token(char_lit));
    g.highlight(hl(Hc::String).token(char_open));
    g.highlight(hl(Hc::Comment).token(line_comment));
    g.highlight(hl(Hc::Comment).token(block_comment));
    for op in [
        eq, eqeq, neq, le, ge, lt, gt, plus, minus, star, slash, percent, andand, oror, bang,
        arrow, dot, dotdot, coloncolon,
    ] {
        g.highlight(hl(Hc::Operator).token(op));
    }
    g.highlight(hl(Hc::Variable).token(ident));

    Arc::new(g.build().expect("the kove grammar is valid"))
}

/// Parse a source text into a fresh [`Document`].
pub fn parse(text: &str) -> Document {
    Document::new(language(), text)
}

/// All syntax-level diagnostics for a document, with Kove error codes:
/// the tree's recovery/lex diagnostics plus scans the tree structure can't
/// express (unterminated strings, chars and block comments).
pub fn syntax_diagnostics(doc: &Document) -> Vec<Diagnostic> {
    let lang = doc.language().clone();
    let text = doc.text();
    let mut out = Vec::new();

    for d in doc.diagnostics() {
        let span = Span::new(d.range.start, d.range.end);
        out.push(match d.message {
            DiagMessage::ExpectedToken(k) => {
                Diagnostic::error("E0101", format!("expected `{}`", lang.token_name(k)), span)
                    .with_label(format!("`{}` expected here", lang.token_name(k)))
            }
            DiagMessage::ExpectedExpression => {
                Diagnostic::error("E0102", "expected an expression", span)
                    .with_label("an expression is missing here")
            }
            DiagMessage::UnexpectedInput => {
                Diagnostic::error("E0103", "unexpected input", span)
                    .with_label("the parser could not make sense of this")
            }
            DiagMessage::UnrecognizedCharacter => {
                let ch = text[d.range.start as usize..d.range.end as usize]
                    .chars()
                    .next()
                    .unwrap_or('?');
                Diagnostic::error(
                    "E0001",
                    format!("unrecognized character `{}`", ch.escape_default()),
                    span,
                )
                .with_label("this character is not part of the Kove language")
            }
        });
    }

    for elem in doc.tree().root().descendants() {
        let SyntaxElem::Token(tok) = elem else {
            continue;
        };
        match lang.token_name(tok.kind()) {
            "unterminated_string" if !tok.is_missing() => {
                let r = tok.text_range();
                out.push(
                    Diagnostic::error(
                        "E0112",
                        "unterminated string literal",
                        Span::new(r.start, r.end),
                    )
                    .with_label("the string starts here but never closes")
                    .with_help("add a closing `\"`"),
                );
            }
            "unterminated_char" if !tok.is_missing() => {
                let r = tok.text_range();
                out.push(
                    Diagnostic::error(
                        "E0113",
                        "unterminated character literal",
                        Span::new(r.start, r.end),
                    )
                    .with_help("add a closing `'`"),
                );
            }
            _ => {}
        }
        for (piece, range) in tok.trivia() {
            if lang.token_name(piece.kind) == "block_comment" {
                let s = &text[range.start as usize..range.end as usize];
                if !(s.len() >= 4 && s.ends_with("*/")) {
                    out.push(
                        Diagnostic::error(
                            "E0114",
                            "unterminated block comment",
                            Span::new(range.start, range.start + 2),
                        )
                        .with_label("the comment opened here is never closed")
                        .with_help("close it with `*/`"),
                    );
                }
            }
        }
    }

    out.sort_by_key(|d| (d.span.start, d.span.end));
    out
}
