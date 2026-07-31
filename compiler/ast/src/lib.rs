//! The Kove AST: a clean representation independent of the concrete syntax
//! tree, with source spans preserved on every node so later stages can
//! always point diagnostics back at source code.
//!
//! [`lower`] converts a parsed [`reparse::Document`] into a [`Program`].
//! Lowering never fails: malformed subtrees (which have already produced
//! syntax diagnostics) become [`ExprKind::Error`] or are skipped, and the
//! driver does not type-check a program whose syntax has errors.

mod lower;

pub use lower::{lower, LowerResult};

use kove_diagnostics::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
    Struct(StructDecl),
    Enum(EnumDecl),
    Import(Import),
}

#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: Ident,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name: Ident,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        mutable: bool,
        name: Ident,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
        span: Span,
    },
    If(IfStmt),
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    For {
        var: Ident,
        iter: Expr,
        body: Block,
        span: Span,
    },
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub cond: Expr,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ElseBranch {
    If(Box<IfStmt>),
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    /// A variable (or, at a call's callee position, a function) reference.
    Var(String),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        op_span: Span,
    },
    /// `lo..hi`, a half-open Int range. Only meaningful as a `for` iterable.
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Field {
        base: Box<Expr>,
        name: Ident,
    },
    StructLit {
        name: Ident,
        fields: Vec<(Ident, Expr)>,
    },
    /// `Enum::Variant`
    Path {
        base: Ident,
        name: Ident,
    },
    /// Placeholder for a malformed subtree that already has a syntax
    /// diagnostic. Never reaches the type checker on a clean parse.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }
}

impl UnaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
        }
    }
}
