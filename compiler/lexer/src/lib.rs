//! Kove's lexical layer: what counts as a token.
//!
//! This crate owns the token vocabulary and nothing else. It answers "is
//! `1..10` one token or three", "is `intx` a keyword", "how far does an
//! unterminated string reach". It has no opinion about how tokens may be
//! arranged; that is `kove-parser`.
//!
//! The tokens are registered into a ReParse [`GrammarBuilder`], because
//! ReParse builds one `Language` covering both lexing and parsing. The
//! split is still real: [`register`] is the only place token patterns are
//! defined, and the parser refers to them exclusively through the
//! [`Tokens`] handle it gets back.

use kove_diagnostics::{Diagnostic, Span};
use reparse::grammar::{hl, GrammarBuilder, TokenKind};
use reparse::highlight::HighlightClass as Hc;
use reparse::{Document, SyntaxElem};

/// Token names, so downstream stages match on one shared vocabulary
/// rather than scattered string literals.
pub mod names {
    pub const IDENTIFIER: &str = "identifier";
    pub const INT: &str = "int";
    pub const FLOAT: &str = "float";
    pub const STRING: &str = "string";
    pub const UNTERMINATED_STRING: &str = "unterminated_string";
    pub const CHAR: &str = "char";
    pub const UNTERMINATED_CHAR: &str = "unterminated_char";
    pub const TRUE: &str = "true";
    pub const FALSE: &str = "false";
    pub const LINE_COMMENT: &str = "line_comment";
    pub const BLOCK_COMMENT: &str = "block_comment";
}

/// Every token kind in the language, handed to the parser so grammar rules
/// can refer to tokens without knowing their patterns.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    // Trivia
    pub line_comment: TokenKind,
    pub block_comment: TokenKind,
    // Literals and names
    pub ident: TokenKind,
    pub int: TokenKind,
    pub float: TokenKind,
    pub string: TokenKind,
    pub string_open: TokenKind,
    pub char_lit: TokenKind,
    pub char_open: TokenKind,
    // Punctuation
    pub lparen: TokenKind,
    pub rparen: TokenKind,
    pub lbrace: TokenKind,
    pub rbrace: TokenKind,
    pub comma: TokenKind,
    pub semi: TokenKind,
    pub colon: TokenKind,
    pub coloncolon: TokenKind,
    pub dot: TokenKind,
    pub dotdot: TokenKind,
    pub arrow: TokenKind,
    // Operators
    pub eq: TokenKind,
    // Compound assignment
    pub plus_eq: TokenKind,
    pub minus_eq: TokenKind,
    pub star_eq: TokenKind,
    pub slash_eq: TokenKind,
    pub percent_eq: TokenKind,
    pub eqeq: TokenKind,
    pub neq: TokenKind,
    pub lt: TokenKind,
    pub le: TokenKind,
    pub gt: TokenKind,
    pub ge: TokenKind,
    pub plus: TokenKind,
    pub minus: TokenKind,
    pub star: TokenKind,
    pub slash: TokenKind,
    pub percent: TokenKind,
    pub andand: TokenKind,
    pub oror: TokenKind,
    pub bang: TokenKind,
    // Keywords
    pub kw_fn: TokenKind,
    pub kw_let: TokenKind,
    pub kw_mut: TokenKind,
    pub kw_return: TokenKind,
    pub kw_if: TokenKind,
    pub kw_else: TokenKind,
    pub kw_while: TokenKind,
    pub kw_for: TokenKind,
    pub kw_in: TokenKind,
    pub kw_struct: TokenKind,
    pub kw_enum: TokenKind,
    pub kw_import: TokenKind,
    pub kw_true: TokenKind,
    pub kw_false: TokenKind,
    pub kw_match: TokenKind,
}

impl Tokens {
    /// Every keyword, for highlighting and for documenting the reserved
    /// word list in one place.
    pub fn keywords(&self) -> [TokenKind; 13] {
        [
            self.kw_fn,
            self.kw_let,
            self.kw_mut,
            self.kw_return,
            self.kw_if,
            self.kw_else,
            self.kw_while,
            self.kw_for,
            self.kw_in,
            self.kw_struct,
            self.kw_enum,
            self.kw_import,
            self.kw_match,
        ]
    }

    /// Every token that assigns, `=` and the compound forms.
    pub fn assignment_operators(&self) -> [TokenKind; 6] {
        [
            self.eq,
            self.plus_eq,
            self.minus_eq,
            self.star_eq,
            self.slash_eq,
            self.percent_eq,
        ]
    }

    /// Every operator token, for highlighting.
    pub fn operators(&self) -> [TokenKind; 24] {
        [
            self.eq,
            self.plus_eq,
            self.minus_eq,
            self.star_eq,
            self.slash_eq,
            self.percent_eq,
            self.eqeq,
            self.neq,
            self.le,
            self.ge,
            self.lt,
            self.gt,
            self.plus,
            self.minus,
            self.star,
            self.slash,
            self.percent,
            self.andand,
            self.oror,
            self.bang,
            self.arrow,
            self.dot,
            self.dotdot,
            self.coloncolon,
        ]
    }
}

/// Define Kove's trivia and tokens on a fresh grammar builder.
///
/// The lexer matches longest-first, which is what makes `1.5` a single
/// float while `1..10` is int, `..`, int, and what keeps `intx` an
/// identifier rather than the keyword `in` followed by `tx`.
pub fn register(g: &mut GrammarBuilder) -> Tokens {
    g.trivia("whitespace", r"[ \t\r\n]+")
        .expect("whitespace pattern is valid");
    let line_comment = g
        .trivia(names::LINE_COMMENT, r"//[^\n]*")
        .expect("line comment pattern is valid");
    // The pattern admits the unterminated form so that an open `/*` has a
    // defined extent instead of a heuristic one. `lex_diagnostics` reports
    // the unclosed case as E0114.
    let block_comment = g
        .trivia(names::BLOCK_COMMENT, r"/\*([^*]|\*(?!/))*(\*/)?")
        .expect("block comment pattern is valid");

    let ident = g
        .token(names::IDENTIFIER, r"[A-Za-z_]\w*")
        .expect("identifier pattern is valid");
    let float = g
        .token(names::FLOAT, r"\d+\.\d+")
        .expect("float pattern is valid");
    let int = g.token(names::INT, r"\d+").expect("int pattern is valid");
    let string = g
        .token(names::STRING, r#""([^"\\\n]|\\.)*""#)
        .expect("string pattern is valid");
    // Unterminated literals get their own tokens so a stray quote cannot
    // swallow the rest of the file, and so edits near one only invalidate
    // a bounded region.
    let string_open = g
        .token(names::UNTERMINATED_STRING, r#""([^"\\\n]|\\.)*\\?"#)
        .expect("unterminated string pattern is valid");
    // The `\u{...}` alternative has to come before `\\.`, which would
    // otherwise match just the `\u` and leave the braces outside.
    let char_lit = g
        .token(names::CHAR, r"'([^'\\\n]|\\u\{[0-9a-fA-F]+\}|\\.)'")
        .expect("char pattern is valid");
    let char_open = g
        .token(names::UNTERMINATED_CHAR, r"'([^'\\\n]|\\.)?")
        .expect("unterminated char pattern is valid");

    Tokens {
        line_comment,
        block_comment,
        ident,
        int,
        float,
        string,
        string_open,
        char_lit,
        char_open,
        lparen: g.punct("("),
        rparen: g.punct(")"),
        lbrace: g.punct("{"),
        rbrace: g.punct("}"),
        comma: g.punct(","),
        semi: g.punct(";"),
        colon: g.punct(":"),
        coloncolon: g.punct("::"),
        dot: g.punct("."),
        dotdot: g.punct(".."),
        arrow: g.punct("->"),
        eq: g.punct("="),
        // Longest match keeps these one token, so `x += 1` never lexes as
        // `x`, `+`, `=`.
        plus_eq: g.punct("+="),
        minus_eq: g.punct("-="),
        star_eq: g.punct("*="),
        slash_eq: g.punct("/="),
        percent_eq: g.punct("%="),
        eqeq: g.punct("=="),
        neq: g.punct("!="),
        le: g.punct("<="),
        ge: g.punct(">="),
        lt: g.punct("<"),
        gt: g.punct(">"),
        plus: g.punct("+"),
        minus: g.punct("-"),
        star: g.punct("*"),
        slash: g.punct("/"),
        percent: g.punct("%"),
        andand: g.punct("&&"),
        oror: g.punct("||"),
        bang: g.punct("!"),
        kw_fn: g.keyword(ident, "fn"),
        kw_let: g.keyword(ident, "let"),
        kw_mut: g.keyword(ident, "mut"),
        kw_return: g.keyword(ident, "return"),
        kw_if: g.keyword(ident, "if"),
        kw_else: g.keyword(ident, "else"),
        kw_while: g.keyword(ident, "while"),
        kw_for: g.keyword(ident, "for"),
        kw_in: g.keyword(ident, "in"),
        kw_struct: g.keyword(ident, "struct"),
        kw_enum: g.keyword(ident, "enum"),
        kw_import: g.keyword(ident, "import"),
        kw_true: g.keyword(ident, names::TRUE),
        kw_false: g.keyword(ident, names::FALSE),
        // Reserved for pattern matching (docs/language.md). It has no
        // grammar yet, but reserving it now keeps `match` from becoming a
        // valid identifier that later breaks.
        kw_match: g.keyword(ident, "match"),
    }
}

/// Token-level highlighting: keywords, literals, comments, operators and
/// the fallback for plain identifiers.
///
/// Call this *after* the parser's context-sensitive rules, because ReParse
/// takes the first matching rule and these are the catch-alls.
pub fn register_highlights(g: &mut GrammarBuilder, t: &Tokens) {
    for kw in t.keywords() {
        g.highlight(hl(Hc::Keyword).token(kw));
    }
    for c in [t.kw_true, t.kw_false] {
        g.highlight(hl(Hc::Constant).token(c));
    }
    g.highlight(hl(Hc::Number).token(t.int));
    g.highlight(hl(Hc::Number).token(t.float));
    for s in [t.string, t.string_open, t.char_lit, t.char_open] {
        g.highlight(hl(Hc::String).token(s));
    }
    g.highlight(hl(Hc::Comment).token(t.line_comment));
    g.highlight(hl(Hc::Comment).token(t.block_comment));
    for op in t.operators() {
        g.highlight(hl(Hc::Operator).token(op));
    }
    g.highlight(hl(Hc::Variable).token(t.ident));
}

/// Lexical diagnostics: problems with the tokens themselves, as opposed to
/// how they are arranged. Unterminated literals and comments are found by
/// scanning the token stream; unrecognized characters come from ReParse's
/// own lexer.
pub fn lex_diagnostics(doc: &Document) -> Vec<Diagnostic> {
    let lang = doc.language().clone();
    let text = doc.text();
    let mut out = Vec::new();

    for d in doc.diagnostics() {
        if matches!(
            d.message,
            reparse::green::DiagMessage::UnrecognizedCharacter
        ) {
            let ch = text[d.range.start as usize..d.range.end as usize]
                .chars()
                .next()
                .unwrap_or('?');
            out.push(
                Diagnostic::error(
                    "E0001",
                    format!("unrecognized character `{}`", ch.escape_default()),
                    Span::new(d.range.start, d.range.end),
                )
                .with_label("this character is not part of the Kove language"),
            );
        }
    }

    for elem in doc.tree().root().descendants() {
        let SyntaxElem::Token(tok) = elem else {
            continue;
        };
        match lang.token_name(tok.kind()) {
            names::UNTERMINATED_STRING if !tok.is_missing() => {
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
            names::UNTERMINATED_CHAR if !tok.is_missing() => {
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
            if lang.token_name(piece.kind) == names::BLOCK_COMMENT {
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

    out
}
