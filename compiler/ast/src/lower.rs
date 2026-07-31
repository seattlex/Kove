//! Lowering: concrete syntax tree -> AST.
//!
//! Works entirely through node kind names and field names declared in
//! `kove-parser`, and token names from `kove-lexer`. The only diagnostics
//! produced here are the ones that
//! need literal decoding (integer overflow, bad escapes) or item-level
//! placement checks (statements at the top level); everything about token
//! shapes was already reported by `syntax_diagnostics`.

use crate::*;
use kove_diagnostics::{Diagnostic, Span};
use kove_lexer::names as tok;
use reparse::grammar::Language;
use reparse::node::{SyntaxElem, SyntaxNode, SyntaxToken};
use reparse::text::TextRange;
use reparse::Document;

pub struct LowerResult {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lower(doc: &Document) -> LowerResult {
    let lang = doc.language().clone();
    let mut l = Lowerer {
        lang: &lang,
        text: doc.text(),
        diags: Vec::new(),
        next_id: 0,
    };
    let program = l.program(&doc.tree().root());
    LowerResult {
        program,
        diagnostics: l.diags,
    }
}

struct Lowerer<'a> {
    lang: &'a Language,
    text: &'a str,
    diags: Vec<Diagnostic>,
    next_id: u32,
}

fn span(r: TextRange) -> Span {
    Span::new(r.start, r.end)
}

impl<'a> Lowerer<'a> {
    // The returned names borrow from the `Language`, not from `self`, so
    // match arms over them are free to push diagnostics.
    fn kind(&self, node: &SyntaxNode) -> &'a str {
        let lang = self.lang;
        lang.node_name(node.kind())
    }

    fn token_kind(&self, tok: &SyntaxToken) -> &'a str {
        let lang = self.lang;
        lang.token_name(tok.kind())
    }

    fn next_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    fn field(&self, node: &SyntaxNode, name: &str) -> Option<SyntaxElem> {
        node.child_by_field(self.lang.field(name)?)
    }

    fn field_token(&self, node: &SyntaxNode, name: &str) -> Option<SyntaxToken> {
        match self.field(node, name)? {
            SyntaxElem::Token(t) if !t.is_missing() => Some(t),
            _ => None,
        }
    }

    fn field_node(&self, node: &SyntaxNode, name: &str) -> Option<SyntaxNode> {
        match self.field(node, name)? {
            SyntaxElem::Node(n) => Some(n),
            _ => None,
        }
    }

    fn ident(&mut self, tok: &SyntaxToken) -> Ident {
        Ident {
            id: self.next_id(),
            name: tok.text(self.text).to_string(),
            span: span(tok.text_range()),
        }
    }

    fn field_ident(&mut self, node: &SyntaxNode, name: &str) -> Option<Ident> {
        let tok = self.field_token(node, name)?;
        Some(self.ident(&tok))
    }

    fn type_expr(&self, node: &SyntaxNode) -> TypeExpr {
        match self.field_token(node, "name") {
            Some(t) => TypeExpr {
                name: t.text(self.text).to_string(),
                span: span(t.text_range()),
            },
            None => TypeExpr {
                name: "<error>".to_string(),
                span: span(node.trimmed_range()),
            },
        }
    }

    fn field_type(&self, node: &SyntaxNode, name: &str) -> Option<TypeExpr> {
        self.field_node(node, name).map(|n| self.type_expr(&n))
    }

    // --- Items ------------------------------------------------------------

    fn program(&mut self, root: &SyntaxNode) -> Program {
        // root is ROOT wrapping source_file (or source_file directly).
        let source = root
            .child_nodes()
            .find(|n| self.kind(n) == "source_file")
            .unwrap_or_else(|| root.clone());
        let mut items = Vec::new();
        for node in source.child_nodes() {
            match self.kind(&node) {
                "function_declaration" => {
                    if let Some(f) = self.function(&node) {
                        items.push(Item::Function(f));
                    }
                }
                "struct_declaration" => {
                    if let Some(s) = self.struct_decl(&node) {
                        items.push(Item::Struct(s));
                    }
                }
                "enum_declaration" => {
                    if let Some(e) = self.enum_decl(&node) {
                        items.push(Item::Enum(e));
                    }
                }
                "import_declaration" => items.push(Item::Import(self.import_decl(&node))),
                "ERROR" => {}
                // A statement at the top level: the grammar accepts it so we
                // can explain the rule instead of emitting "unexpected input".
                _ => {
                    self.diags.push(
                        Diagnostic::error(
                            "E0104",
                            "statements are not allowed at the top level",
                            span(node.trimmed_range()),
                        )
                        .with_label("this code is outside of any function")
                        .with_help("move it into a function, for example `fn main() { ... }`"),
                    );
                }
            }
        }
        Program { items }
    }

    fn function(&mut self, node: &SyntaxNode) -> Option<Function> {
        let name = self.field_ident(node, "name")?;
        let mut params = Vec::new();
        if let Some(list) = self.field_node(node, "params") {
            for p in list.child_nodes() {
                if self.kind(&p) != "parameter" {
                    continue;
                }
                let Some(pname) = self.field_ident(&p, "name") else {
                    continue;
                };
                let ty = self
                    .field_type(&p, "type")
                    .unwrap_or_else(|| error_type(span(p.trimmed_range())));
                params.push(Param { name: pname, ty });
            }
        }
        let return_type = self.field_type(node, "return_type");
        let body = match self.field_node(node, "body") {
            Some(b) => self.block(&b),
            None => Block {
                stmts: Vec::new(),
                span: span(node.trimmed_range()),
            },
        };
        Some(Function {
            name,
            params,
            return_type,
            body,
            span: span(node.trimmed_range()),
        })
    }

    fn struct_decl(&mut self, node: &SyntaxNode) -> Option<StructDecl> {
        let name = self.field_ident(node, "name")?;
        let mut fields = Vec::new();
        for f in node.child_nodes() {
            if self.kind(&f) != "field_declaration" {
                continue;
            }
            let Some(fname) = self.field_ident(&f, "name") else {
                continue;
            };
            let ty = self
                .field_type(&f, "type")
                .unwrap_or_else(|| error_type(span(f.trimmed_range())));
            fields.push(FieldDecl { name: fname, ty });
        }
        Some(StructDecl {
            name,
            fields,
            span: span(node.trimmed_range()),
        })
    }

    fn enum_decl(&mut self, node: &SyntaxNode) -> Option<EnumDecl> {
        let name = self.field_ident(node, "name")?;
        let mut variants = Vec::new();
        for v in node.child_nodes() {
            if self.kind(&v) != "variant" {
                continue;
            }
            if let Some(vname) = self.field_ident(&v, "name") {
                variants.push(vname);
            }
        }
        Some(EnumDecl {
            name,
            variants,
            span: span(node.trimmed_range()),
        })
    }

    fn import_decl(&mut self, node: &SyntaxNode) -> Import {
        let mut path = Vec::new();
        if let Some(p) = self.field_node(node, "path") {
            for elem in p.children() {
                if let SyntaxElem::Token(t) = elem {
                    if self.token_kind(&t) == tok::IDENTIFIER && !t.is_missing() {
                        path.push(self.ident(&t));
                    }
                }
            }
        }
        Import {
            path,
            span: span(node.trimmed_range()),
        }
    }

    // --- Statements -------------------------------------------------------

    fn block(&mut self, node: &SyntaxNode) -> Block {
        let mut stmts = Vec::new();
        for child in node.child_nodes() {
            if let Some(s) = self.stmt(&child) {
                stmts.push(s);
            }
        }
        Block {
            stmts,
            span: span(node.trimmed_range()),
        }
    }

    fn stmt(&mut self, node: &SyntaxNode) -> Option<Stmt> {
        let sp = span(node.trimmed_range());
        match self.kind(node) {
            "let_statement" => {
                let mutable = self.field(node, "mutable").is_some();
                let name = self.field_ident(node, "name")?;
                let ty = self.field_type(node, "type");
                let value = self.expr_field(node, "value");
                Some(Stmt::Let {
                    mutable,
                    name,
                    ty,
                    value,
                    span: sp,
                })
            }
            "assignment_statement" => Some(Stmt::Assign {
                target: self.expr_field(node, "target"),
                value: self.expr_field(node, "value"),
                span: sp,
            }),
            "return_statement" => Some(Stmt::Return {
                value: self.field(node, "value").map(|e| self.expr(&e)),
                span: sp,
            }),
            "if_statement" => self.if_stmt(node).map(Stmt::If),
            "while_statement" => Some(Stmt::While {
                cond: self.expr_field(node, "condition"),
                body: self
                    .field_node(node, "body")
                    .map(|b| self.block(&b))
                    .unwrap_or(Block {
                        stmts: Vec::new(),
                        span: sp,
                    }),
                span: sp,
            }),
            "for_statement" => Some(Stmt::For {
                var: self.field_ident(node, "name")?,
                iter: self.expr_field(node, "iter"),
                body: self
                    .field_node(node, "body")
                    .map(|b| self.block(&b))
                    .unwrap_or(Block {
                        stmts: Vec::new(),
                        span: sp,
                    }),
                span: sp,
            }),
            "block" => Some(Stmt::Block(self.block(node))),
            "expression_statement" => Some(Stmt::Expr(self.expr_field(node, "value"))),
            _ => None, // ERROR islands: already diagnosed by the parser.
        }
    }

    fn if_stmt(&mut self, node: &SyntaxNode) -> Option<IfStmt> {
        let sp = span(node.trimmed_range());
        let cond = self.expr_field(node, "condition");
        let then_block = self
            .field_node(node, "then")
            .map(|b| self.block(&b))
            .unwrap_or(Block {
                stmts: Vec::new(),
                span: sp,
            });
        let else_branch = self.field_node(node, "else").and_then(|e| {
            if self.kind(&e) == "if_statement" {
                self.if_stmt(&e).map(|i| ElseBranch::If(Box::new(i)))
            } else {
                Some(ElseBranch::Block(self.block(&e)))
            }
        });
        Some(IfStmt {
            cond,
            then_block,
            else_branch,
            span: sp,
        })
    }

    // --- Expressions ------------------------------------------------------

    /// Lower the expression stored in `field`, or an error placeholder when
    /// it is absent (in which case a syntax diagnostic already exists).
    fn expr_field(&mut self, node: &SyntaxNode, field: &str) -> Expr {
        match self.field(node, field) {
            Some(e) => self.expr(&e),
            None => Expr {
                id: self.next_id(),
                kind: ExprKind::Error,
                span: span(node.trimmed_range()),
            },
        }
    }

    fn expr(&mut self, elem: &SyntaxElem) -> Expr {
        match elem {
            SyntaxElem::Token(tok) => self.token_expr(tok),
            SyntaxElem::Node(node) => self.node_expr(node),
        }
    }

    fn token_expr(&mut self, tok: &SyntaxToken) -> Expr {
        let sp = span(tok.text_range());
        if tok.is_missing() {
            return Expr {
                id: self.next_id(),
                kind: ExprKind::Error,
                span: sp,
            };
        }
        let text = tok.text(self.text);
        let kind = match self.token_kind(tok) {
            tok::INT => match text.parse::<i64>() {
                Ok(v) => ExprKind::Int(v),
                Err(_) => {
                    self.diags.push(
                        Diagnostic::error("E0110", "integer literal is too large", sp)
                            .with_label("does not fit in `Int`")
                            .with_note(format!("the largest `Int` value is {}", i64::MAX)),
                    );
                    ExprKind::Error
                }
            },
            tok::FLOAT => match text.parse::<f64>() {
                Ok(v) => ExprKind::Float(v),
                Err(_) => ExprKind::Error,
            },
            tok::STRING => ExprKind::Str(self.unescape(&text[1..text.len() - 1], sp.start + 1)),
            tok::UNTERMINATED_STRING => {
                // Already reported as E0112; decode what is there.
                ExprKind::Str(self.unescape(&text[1..], sp.start + 1))
            }
            tok::CHAR => {
                let inner = self.unescape(&text[1..text.len() - 1], sp.start + 1);
                match inner.chars().next() {
                    Some(c) => ExprKind::Char(c),
                    None => ExprKind::Error,
                }
            }
            tok::UNTERMINATED_CHAR => ExprKind::Error, // already reported as E0113
            tok::TRUE => ExprKind::Bool(true),
            tok::FALSE => ExprKind::Bool(false),
            tok::IDENTIFIER => ExprKind::Var(text.to_string()),
            _ => ExprKind::Error,
        };
        Expr {
            id: self.next_id(),
            kind,
            span: sp,
        }
    }

    fn node_expr(&mut self, node: &SyntaxNode) -> Expr {
        let sp = span(node.trimmed_range());
        let kind = match self.kind(node) {
            "binary_expression" => {
                let lhs = self.expr_field(node, "lhs");
                let rhs = self.expr_field(node, "rhs");
                match self.field_token(node, "op") {
                    Some(op_tok) => {
                        let op = binary_op(op_tok.text(self.text));
                        match op {
                            Some(op) => ExprKind::Binary {
                                op,
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs),
                                op_span: span(op_tok.text_range()),
                            },
                            None => ExprKind::Error,
                        }
                    }
                    None => ExprKind::Error,
                }
            }
            "range_expression" => ExprKind::Range {
                lo: Box::new(self.expr_field(node, "lhs")),
                hi: Box::new(self.expr_field(node, "rhs")),
            },
            "unary_expression" => {
                let operand = self.expr_field(node, "operand");
                match self.field_token(node, "op") {
                    Some(op_tok) => {
                        let op = if op_tok.text(self.text) == "-" {
                            UnaryOp::Neg
                        } else {
                            UnaryOp::Not
                        };
                        ExprKind::Unary {
                            op,
                            operand: Box::new(operand),
                        }
                    }
                    None => ExprKind::Error,
                }
            }
            "call_expression" => {
                let callee = self.expr_field(node, "callee");
                let mut args = Vec::new();
                if let Some(arg_field) = self.lang.field("arg") {
                    for argument_list in node.child_nodes() {
                        if self.kind(&argument_list) != "arguments" {
                            continue;
                        }
                        for child in argument_list.children() {
                            if child.field() == Some(arg_field) {
                                args.push(self.expr(&child));
                            }
                        }
                    }
                }
                ExprKind::Call {
                    callee: Box::new(callee),
                    args,
                }
            }
            "field_expression" => match self.field_ident(node, "name") {
                Some(name) => ExprKind::Field {
                    base: Box::new(self.expr_field(node, "base")),
                    name,
                },
                None => ExprKind::Error,
            },
            "struct_literal" => match self.field_ident(node, "name") {
                Some(name) => {
                    let mut fields = Vec::new();
                    for init in node.child_nodes() {
                        if self.kind(&init) != "field_initializer" {
                            continue;
                        }
                        if let Some(fname) = self.field_ident(&init, "name") {
                            let value = self.expr_field(&init, "value");
                            fields.push((fname, value));
                        }
                    }
                    ExprKind::StructLit { name, fields }
                }
                None => ExprKind::Error,
            },
            "path_expression" => {
                match (
                    self.field_ident(node, "base"),
                    self.field_ident(node, "name"),
                ) {
                    (Some(base), Some(name)) => ExprKind::Path { base, name },
                    _ => ExprKind::Error,
                }
            }
            "paren_expression" => {
                return match self.field(node, "inner") {
                    Some(inner) => self.expr(&inner),
                    None => Expr {
                        id: self.next_id(),
                        kind: ExprKind::Error,
                        span: sp,
                    },
                };
            }
            _ => ExprKind::Error,
        };
        Expr {
            id: self.next_id(),
            kind,
            span: sp,
        }
    }

    /// Decode escape sequences. `base` is the byte offset of the first
    /// character inside the quotes, for precise error spans.
    fn unescape(&mut self, raw: &str, base: u32) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut chars = raw.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, 'r')) => out.push('\r'),
                Some((_, '0')) => out.push('\0'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, '"')) => out.push('"'),
                Some((_, '\'')) => out.push('\''),
                Some((j, other)) => {
                    let start = base + i as u32;
                    let end = base + j as u32 + other.len_utf8() as u32;
                    self.diags.push(
                        Diagnostic::error(
                            "E0111",
                            format!("unknown escape sequence `\\{}`", other.escape_default()),
                            Span::new(start, end),
                        )
                        .with_help("supported escapes are \\n, \\t, \\r, \\0, \\\\, \\\" and \\'"),
                    );
                    out.push(other);
                }
                None => {}
            }
        }
        out
    }
}

fn error_type(span: Span) -> TypeExpr {
    TypeExpr {
        name: "<error>".to_string(),
        span,
    }
}

fn binary_op(text: &str) -> Option<BinaryOp> {
    Some(match text {
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Sub,
        "*" => BinaryOp::Mul,
        "/" => BinaryOp::Div,
        "%" => BinaryOp::Rem,
        "==" => BinaryOp::Eq,
        "!=" => BinaryOp::Ne,
        "<" => BinaryOp::Lt,
        "<=" => BinaryOp::Le,
        ">" => BinaryOp::Gt,
        ">=" => BinaryOp::Ge,
        "&&" => BinaryOp::And,
        "||" => BinaryOp::Or,
        _ => return None,
    })
}
