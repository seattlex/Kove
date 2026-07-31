//! Kove's grammar: how tokens may be arranged.
//!
//! Tokens come from `kove-lexer`; this crate adds the rules, the recovery
//! policy and the editor annotations, and produces the concrete syntax
//! tree. It is the last stage that knows about surface syntax. Everything
//! downstream reads the tree through the node and field names declared
//! here.
//!
//! The engine is [ReParse](https://github.com/seattlex/ReParse), so the
//! tree is incremental and always complete: broken input still produces a
//! tree covering every byte, with zero-width missing tokens and error
//! islands where things went wrong. That is what lets `kove check` report
//! several syntax errors at once, and what will let the language server
//! reuse this exact frontend.

use kove_diagnostics::{Diagnostic, Span};
use kove_lexer::Tokens;
use reparse::grammar::*;
use reparse::green::DiagMessage;
use reparse::highlight::HighlightClass as Hc;
use reparse::Document;
use std::sync::{Arc, OnceLock};

pub use kove_lexer;
pub use reparse;

/// The Kove language definition. Built once, shared everywhere.
pub fn language() -> Arc<Language> {
    static LANG: OnceLock<Arc<Language>> = OnceLock::new();
    LANG.get_or_init(build_language).clone()
}

fn build_language() -> Arc<Language> {
    let mut g = GrammarBuilder::new("kove");

    // The lexer owns every token pattern; `k` is how rules name them.
    let k: Tokens = kove_lexer::register(&mut g);

    // Tokens recovery may insert as zero-width "missing" leaves. Keeping
    // this list small means recovery only ever invents punctuation whose
    // absence is unambiguous.
    g.recover_tokens(&[k.rparen, k.rbrace, k.comma, k.semi]);

    // --- Expressions ------------------------------------------------------
    let expression = g.declare("expression");
    g.make_inline(expression);
    let unary = g.declare("unary");
    g.make_inline(unary);
    let postfix_expr = g.declare("postfix_expr");
    g.make_inline(postfix_expr);
    let block = g.declare("block");
    let if_stmt = g.declare("if_statement");
    let type_expr = g.rule("type_expression", field("name", t(k.ident)));

    let arguments = g.rule(
        "arguments",
        seq(vec![
            t(k.lparen),
            rep0(field("arg", r(expression)))
                .sep(t(k.comma))
                .allow_trailing()
                .recover(&[k.rparen, k.semi, k.rbrace]),
            t(k.rparen),
        ]),
    );

    let paren_expr = g.rule(
        "paren_expression",
        seq(vec![
            t(k.lparen),
            field("inner", r(expression)),
            t(k.rparen),
        ]),
    );

    // `Enum::Variant`
    let path_expr = g.rule(
        "path_expression",
        seq(vec![
            field("base", t(k.ident)),
            t(k.coloncolon),
            field("name", t(k.ident)),
        ]),
    );

    // `User { name: "Alex", age: 20 }`. Requires at least one field and has
    // no recovery point on purpose: together those guarantee that a block
    // after a bare-identifier condition (`if ready { ... }`) can never be
    // captured as a struct literal. The literal alternative fails and the
    // parser backtracks to the plain identifier.
    let field_init = g.rule(
        "field_initializer",
        seq(vec![
            field("name", t(k.ident)),
            t(k.colon),
            field("value", r(expression)),
        ]),
    );
    let struct_lit = g.rule(
        "struct_literal",
        seq(vec![
            field("name", t(k.ident)),
            t(k.lbrace),
            rep1(r(field_init)).sep(t(k.comma)).allow_trailing(),
            t(k.rbrace),
        ]),
    );

    let primary = g.inline(
        "primary",
        choice(vec![
            t(k.float),
            t(k.int),
            t(k.string),
            t(k.string_open),
            t(k.char_lit),
            t(k.char_open),
            t(k.kw_true),
            t(k.kw_false),
            r(path_expr),
            r(struct_lit),
            t(k.ident),
            r(paren_expr),
        ]),
    );

    let unary_expr = g.rule(
        "unary_expression",
        seq(vec![
            field("op", choice(vec![t(k.bang), t(k.minus)])),
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
                    seq(vec![t(k.dot), field("name", t(k.ident))]),
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
                level(&[k.dotdot], Assoc::Left, "range_expression").fields("lhs", "op", "rhs"),
                level(&[k.oror], Assoc::Left, "binary_expression").fields("lhs", "op", "rhs"),
                level(&[k.andand], Assoc::Left, "binary_expression").fields("lhs", "op", "rhs"),
                level(&[k.eqeq, k.neq], Assoc::Left, "binary_expression")
                    .fields("lhs", "op", "rhs"),
                level(&[k.lt, k.gt, k.le, k.ge], Assoc::Left, "binary_expression")
                    .fields("lhs", "op", "rhs"),
                level(&[k.plus, k.minus], Assoc::Left, "binary_expression")
                    .fields("lhs", "op", "rhs"),
                level(
                    &[k.star, k.slash, k.percent],
                    Assoc::Left,
                    "binary_expression",
                )
                .fields("lhs", "op", "rhs"),
            ],
        ),
    );

    // --- Statements -------------------------------------------------------
    let let_stmt = g.rule(
        "let_statement",
        seq(vec![
            t(k.kw_let),
            opt(field("mutable", t(k.kw_mut))),
            field("name", t(k.ident)),
            opt(seq(vec![t(k.colon), field("type", r(type_expr))])),
            t(k.eq),
            field("value", r(expression)),
            t(k.semi),
        ]),
    );

    // `x = v` plus the compound forms. Lowering desugars `x += v` into
    // `x = x + v`, so nothing after the AST knows they exist.
    let assign_stmt = g.rule(
        "assignment_statement",
        seq(vec![
            field("target", r(postfix_expr)),
            field(
                "op",
                choice(k.assignment_operators().iter().copied().map(t).collect()),
            ),
            field("value", r(expression)),
            t(k.semi),
        ]),
    );

    let return_stmt = g.rule(
        "return_statement",
        seq(vec![
            t(k.kw_return),
            opt(field("value", r(expression))),
            t(k.semi),
        ]),
    );

    let while_stmt = g.rule(
        "while_statement",
        seq(vec![
            t(k.kw_while),
            field("condition", r(expression)),
            field("body", r(block)),
        ]),
    );

    let for_stmt = g.rule(
        "for_statement",
        seq(vec![
            t(k.kw_for),
            field("name", t(k.ident)),
            t(k.kw_in),
            field("iter", r(expression)),
            field("body", r(block)),
        ]),
    );

    let expr_stmt = g.rule(
        "expression_statement",
        seq(vec![field("value", r(expression)), t(k.semi)]),
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
            t(k.lbrace),
            rep0(r(statement)).recover(&[k.rbrace]),
            t(k.rbrace),
        ]),
    );

    g.define(
        if_stmt,
        seq(vec![
            t(k.kw_if),
            field("condition", r(expression)),
            field("then", r(block)),
            opt(seq(vec![
                t(k.kw_else),
                field("else", choice(vec![r(if_stmt), r(block)])),
            ])),
        ]),
    );

    // --- Items ------------------------------------------------------------
    let parameter = g.rule(
        "parameter",
        seq(vec![
            field("name", t(k.ident)),
            t(k.colon),
            field("type", r(type_expr)),
        ]),
    );
    let parameter_list = g.rule(
        "parameter_list",
        seq(vec![
            t(k.lparen),
            rep0(r(parameter))
                .sep(t(k.comma))
                .allow_trailing()
                .recover(&[k.rparen, k.lbrace, k.rbrace, k.semi]),
            t(k.rparen),
        ]),
    );

    let function = g.rule(
        "function_declaration",
        seq(vec![
            t(k.kw_fn),
            field("name", t(k.ident)),
            field("params", r(parameter_list)),
            opt(seq(vec![t(k.arrow), field("return_type", r(type_expr))])),
            field("body", r(block)),
        ]),
    );

    let field_decl = g.rule(
        "field_declaration",
        seq(vec![
            field("name", t(k.ident)),
            t(k.colon),
            field("type", r(type_expr)),
        ]),
    );
    let struct_decl = g.rule(
        "struct_declaration",
        seq(vec![
            t(k.kw_struct),
            field("name", t(k.ident)),
            t(k.lbrace),
            rep0(r(field_decl))
                .sep(t(k.comma))
                .allow_trailing()
                .recover(&[k.rbrace]),
            t(k.rbrace),
        ]),
    );

    let variant = g.rule("variant", field("name", t(k.ident)));
    let enum_decl = g.rule(
        "enum_declaration",
        seq(vec![
            t(k.kw_enum),
            field("name", t(k.ident)),
            t(k.lbrace),
            rep0(r(variant))
                .sep(t(k.comma))
                .allow_trailing()
                .recover(&[k.rbrace]),
            t(k.rbrace),
        ]),
    );

    let import_path = g.rule(
        "import_path",
        seq(vec![
            t(k.ident),
            rep0(seq(vec![t(k.coloncolon), t(k.ident)])),
        ]),
    );
    let import_decl = g.rule(
        "import_declaration",
        seq(vec![
            t(k.kw_import),
            field("path", r(import_path)),
            t(k.semi),
        ]),
    );

    // Statements are accepted at the top level so a stray `let` outside a
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

    // Context-sensitive highlighting first: an identifier means different
    // things in a type position, a declaration name, or a call. ReParse
    // takes the first matching rule, so these must precede the lexer's
    // token-level fallbacks registered below.
    g.highlight(hl(Hc::Type).token(k.ident).in_rule(type_expr));
    g.highlight(
        hl(Hc::Type)
            .token(k.ident)
            .field("name")
            .in_rule(struct_decl),
    );
    g.highlight(hl(Hc::Type).token(k.ident).field("name").in_rule(enum_decl));
    g.highlight(
        hl(Hc::Type)
            .token(k.ident)
            .field("name")
            .in_rule(struct_lit),
    );
    g.highlight(hl(Hc::Type).token(k.ident).field("base").in_rule(path_expr));
    g.highlight(
        hl(Hc::Function)
            .token(k.ident)
            .field("name")
            .in_rule(function),
    );
    g.highlight(hl(Hc::Function).token(k.ident).field("callee"));
    g.highlight(hl(Hc::Parameter).token(k.ident).in_rule(parameter));
    g.highlight(
        hl(Hc::Property)
            .token(k.ident)
            .field("name")
            .in_node("field_expression"),
    );
    g.highlight(hl(Hc::Property).token(k.ident).in_rule(field_decl));
    g.highlight(hl(Hc::Property).token(k.ident).in_rule(field_init));
    kove_lexer::register_highlights(&mut g, &k);

    Arc::new(g.build().expect("the kove grammar is valid"))
}

/// Parse a source text into a fresh [`Document`].
pub fn parse(text: &str) -> Document {
    Document::new(language(), text)
}

/// Every syntax-level diagnostic for a document: the lexer's (bad tokens)
/// plus the parser's own (badly arranged ones), in source order.
pub fn syntax_diagnostics(doc: &Document) -> Vec<Diagnostic> {
    let lang = doc.language().clone();
    let mut out = kove_lexer::lex_diagnostics(doc);

    for d in doc.diagnostics() {
        let span = Span::new(d.range.start, d.range.end);
        out.push(match d.message {
            DiagMessage::ExpectedToken(kind) => Diagnostic::error(
                "E0101",
                format!("expected `{}`", lang.token_name(kind)),
                span,
            )
            .with_label(format!("`{}` expected here", lang.token_name(kind))),
            DiagMessage::ExpectedExpression => {
                Diagnostic::error("E0102", "expected an expression", span)
                    .with_label("an expression is missing here")
            }
            DiagMessage::UnexpectedInput => Diagnostic::error("E0103", "unexpected input", span)
                .with_label("the parser could not make sense of this"),
            // Lexical, already reported by `kove_lexer::lex_diagnostics`.
            DiagMessage::UnrecognizedCharacter => continue,
        });
    }

    out.sort_by_key(|d| (d.span.start, d.span.end));
    out
}
