//! Name resolution and type checking.
//!
//! Two passes: [`Checker::collect`] gathers item signatures (struct fields,
//! enum variants, function signatures) so items can reference each other in
//! any order; the second pass checks every function body.
//!
//! Error handling philosophy: a diagnostic is reported once at the point
//! where checking fails and the offending expression gets [`Ty::Error`],
//! which is compatible with everything, so one mistake produces one error
//! instead of a cascade, and the checker keeps going to report every
//! independent error in the file.

use kove_ast::*;
use kove_diagnostics::{Diagnostic, Span};
use std::collections::HashMap;

/// A Kove type. `Struct`/`Enum` index into the checker's item tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    Char,
    Str,
    Unit,
    Struct(usize),
    Enum(usize),
    /// The type of `lo..hi`; only a `for` loop can consume it.
    Range,
    /// An expression that already failed to check; compatible with anything.
    Error,
}

impl Ty {
    fn is_error(self) -> bool {
        matches!(self, Ty::Error)
    }
}

/// Types are compatible if equal or if either side already failed.
fn compat(a: Ty, b: Ty) -> bool {
    a == b || a.is_error() || b.is_error()
}

struct StructInfo {
    name: String,
    span: Span,
    fields: Vec<(String, Ty)>,
}

struct EnumInfo {
    name: String,
    span: Span,
    variants: Vec<String>,
}

struct FuncInfo {
    name: String,
    params: Vec<(String, Ty)>,
    ret: Ty,
}

#[derive(Clone, Copy)]
struct VarInfo {
    ty: Ty,
    mutable: bool,
}

/// Check a whole program, returning every semantic diagnostic found.
pub fn check(program: &Program) -> Vec<Diagnostic> {
    let mut c = Checker::default();
    c.collect(program);
    for item in &program.items {
        if let Item::Function(f) = item {
            c.check_function(f);
        }
    }
    c.diags.sort_by_key(|d| (d.span.start, d.span.end));
    c.diags
}

/// Extra checks for `kove run` / `kove build`: an executable program needs
/// `fn main()` with no parameters and no return type.
pub fn check_main(program: &Program) -> Vec<Diagnostic> {
    let main = program.items.iter().find_map(|i| match i {
        Item::Function(f) if f.name.name == "main" => Some(f),
        _ => None,
    });
    match main {
        None => vec![
            Diagnostic::error("E0214", "no `main` function found", Span::empty(0))
                .with_help("add `fn main() { ... }`; it is where execution starts"),
        ],
        Some(f) if !f.params.is_empty() || f.return_type.is_some() => {
            vec![Diagnostic::error(
                "E0214",
                "`main` must take no parameters and return nothing",
                f.name.span,
            )
            .with_label("declared here")
            .with_help("change the signature to `fn main()`")]
        }
        Some(_) => Vec::new(),
    }
}

#[derive(Default)]
struct Checker {
    structs: Vec<StructInfo>,
    enums: Vec<EnumInfo>,
    funcs: Vec<FuncInfo>,
    struct_by_name: HashMap<String, usize>,
    enum_by_name: HashMap<String, usize>,
    func_by_name: HashMap<String, usize>,
    diags: Vec<Diagnostic>,
}

impl Checker {
    // --- Pass 1: item signatures -----------------------------------------

    fn collect(&mut self, program: &Program) {
        // Names first, so field/parameter types can reference any item.
        for item in &program.items {
            match item {
                Item::Struct(s) => {
                    if self.check_duplicate_type(&s.name) {
                        continue;
                    }
                    let idx = self.structs.len();
                    self.structs.push(StructInfo {
                        name: s.name.name.clone(),
                        span: s.name.span,
                        fields: Vec::new(),
                    });
                    self.struct_by_name.insert(s.name.name.clone(), idx);
                }
                Item::Enum(e) => {
                    if self.check_duplicate_type(&e.name) {
                        continue;
                    }
                    let idx = self.enums.len();
                    self.enums.push(EnumInfo {
                        name: e.name.name.clone(),
                        span: e.name.span,
                        variants: e.variants.iter().map(|v| v.name.clone()).collect(),
                    });
                    self.enum_by_name.insert(e.name.name.clone(), idx);
                }
                Item::Function(f) => {
                    if f.name.name == "println" {
                        self.diags.push(
                            Diagnostic::error(
                                "E0205",
                                "the name `println` is reserved for the built-in print function",
                                f.name.span,
                            )
                            .with_label("cannot be redefined"),
                        );
                        continue;
                    }
                    if let Some(&prev) = self.func_by_name.get(&f.name.name) {
                        let prev_name = self.funcs[prev].name.clone();
                        self.diags.push(
                            Diagnostic::error(
                                "E0205",
                                format!("the function `{}` is defined more than once", prev_name),
                                f.name.span,
                            )
                            .with_label("redefined here")
                            .with_help("give each function a unique name"),
                        );
                        continue;
                    }
                    let idx = self.funcs.len();
                    self.funcs.push(FuncInfo {
                        name: f.name.name.clone(),
                        params: Vec::new(),
                        ret: Ty::Unit,
                    });
                    self.func_by_name.insert(f.name.name.clone(), idx);
                }
                Item::Import(imp) => {
                    let path: Vec<&str> = imp.path.iter().map(|i| i.name.as_str()).collect();
                    self.diags.push(
                        Diagnostic::error(
                            "E0217",
                            format!("the module system is not implemented yet (`import {}`)", path.join("::")),
                            imp.span,
                        )
                        .with_note("modules are planned (see docs/language.md); for now a program is a single file"),
                    );
                }
            }
        }

        // Signatures second.
        for item in &program.items {
            match item {
                Item::Struct(s) => {
                    let Some(&idx) = self.struct_by_name.get(&s.name.name) else {
                        continue;
                    };
                    let mut fields = Vec::new();
                    let mut seen: HashMap<&str, ()> = HashMap::new();
                    for f in &s.fields {
                        if seen.insert(&f.name.name, ()).is_some() {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0205",
                                    format!(
                                        "the field `{}` is declared more than once on struct `{}`",
                                        f.name.name, s.name.name
                                    ),
                                    f.name.span,
                                )
                                .with_label("duplicate field"),
                            );
                            continue;
                        }
                        let ty = self.resolve_type(&f.ty);
                        fields.push((f.name.name.clone(), ty));
                    }
                    self.structs[idx].fields = fields;
                }
                Item::Enum(e) => {
                    let mut seen: HashMap<&str, ()> = HashMap::new();
                    for v in &e.variants {
                        if seen.insert(&v.name, ()).is_some() {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0205",
                                    format!(
                                        "the variant `{}` is declared more than once on enum `{}`",
                                        v.name, e.name.name
                                    ),
                                    v.span,
                                )
                                .with_label("duplicate variant"),
                            );
                        }
                    }
                }
                Item::Function(f) => {
                    let Some(&idx) = self.func_by_name.get(&f.name.name) else {
                        continue;
                    };
                    let mut params = Vec::new();
                    let mut seen: HashMap<&str, ()> = HashMap::new();
                    for p in &f.params {
                        if seen.insert(&p.name.name, ()).is_some() {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0205",
                                    format!("duplicate parameter name `{}`", p.name.name),
                                    p.name.span,
                                )
                                .with_label("already used by an earlier parameter"),
                            );
                        }
                        let ty = self.resolve_type(&p.ty);
                        params.push((p.name.name.clone(), ty));
                    }
                    let ret = match &f.return_type {
                        Some(t) => self.resolve_type(t),
                        None => Ty::Unit,
                    };
                    self.funcs[idx].params = params;
                    self.funcs[idx].ret = ret;
                }
                Item::Import(_) => {}
            }
        }
    }

    /// Reports E0205 and returns true when the type name is already taken.
    fn check_duplicate_type(&mut self, name: &Ident) -> bool {
        let prev_span = self
            .struct_by_name
            .get(&name.name)
            .map(|&i| self.structs[i].span)
            .or_else(|| {
                self.enum_by_name
                    .get(&name.name)
                    .map(|&i| self.enums[i].span)
            });
        if prev_span.is_some() {
            self.diags.push(
                Diagnostic::error(
                    "E0205",
                    format!("the type `{}` is defined more than once", name.name),
                    name.span,
                )
                .with_label("redefined here")
                .with_help("give each struct and enum a unique name"),
            );
            return true;
        }
        false
    }

    fn resolve_type(&mut self, t: &TypeExpr) -> Ty {
        match t.name.as_str() {
            "Int" => Ty::Int,
            "Float" => Ty::Float,
            "Bool" => Ty::Bool,
            "Char" => Ty::Char,
            "String" => Ty::Str,
            "<error>" => Ty::Error, // malformed syntax, already reported
            name => {
                if let Some(&i) = self.struct_by_name.get(name) {
                    Ty::Struct(i)
                } else if let Some(&i) = self.enum_by_name.get(name) {
                    Ty::Enum(i)
                } else {
                    self.diags.push(
                        Diagnostic::error("E0200", format!("cannot find type `{}`", name), t.span)
                            .with_label("not a known type")
                            .with_help(
                                "the primitive types are Int, Float, Bool, Char and String; \
                             structs and enums must be declared in this file",
                            ),
                    );
                    Ty::Error
                }
            }
        }
    }

    fn ty_name(&self, ty: Ty) -> String {
        match ty {
            Ty::Int => "Int".into(),
            Ty::Float => "Float".into(),
            Ty::Bool => "Bool".into(),
            Ty::Char => "Char".into(),
            Ty::Str => "String".into(),
            Ty::Unit => "()".into(),
            Ty::Struct(i) => self.structs[i].name.clone(),
            Ty::Enum(i) => self.enums[i].name.clone(),
            Ty::Range => "Range".into(),
            Ty::Error => "<error>".into(),
        }
    }

    // --- Pass 2: function bodies -----------------------------------------

    fn check_function(&mut self, f: &Function) {
        let Some(&idx) = self.func_by_name.get(&f.name.name) else {
            return; // duplicate definition; the first one was checked
        };
        let ret = self.funcs[idx].ret;
        let mut scopes = Scopes::new();
        scopes.push();
        for (name, ty) in self.funcs[idx].params.clone() {
            scopes.declare(name, VarInfo { ty, mutable: false });
        }
        self.check_block(&f.body, &mut scopes, ret);
        scopes.pop();

        if ret != Ty::Unit && !ret.is_error() && !always_returns(&f.body) {
            self.diags.push(
                Diagnostic::error(
                    "E0210",
                    format!(
                        "not all paths in `{}` return a value of type `{}`",
                        f.name.name,
                        self.ty_name(ret)
                    ),
                    f.name.span,
                )
                .with_label("declared to return a value")
                .with_help("add a `return` at the end of the function, or cover every branch"),
            );
        }
    }

    fn check_block(&mut self, block: &Block, scopes: &mut Scopes, ret: Ty) {
        scopes.push();
        for stmt in &block.stmts {
            self.check_stmt(stmt, scopes, ret);
        }
        scopes.pop();
    }

    fn check_stmt(&mut self, stmt: &Stmt, scopes: &mut Scopes, ret: Ty) {
        match stmt {
            Stmt::Let {
                mutable,
                name,
                ty,
                value,
                ..
            } => {
                let value_ty = self.check_expr(value, scopes);
                let var_ty = match ty {
                    Some(ann) => {
                        let expected = self.resolve_type(ann);
                        if !compat(expected, value_ty) {
                            let mut d = Diagnostic::error(
                                "E0012",
                                format!(
                                    "mismatched types: expected `{}`, found `{}`",
                                    self.ty_name(expected),
                                    self.ty_name(value_ty)
                                ),
                                value.span,
                            )
                            .with_label(format!("expected `{}`", self.ty_name(expected)));
                            d = if expected == Ty::Int && matches!(value.kind, ExprKind::Str(_)) {
                                d.with_help("remove the quotes or change the variable type")
                            } else {
                                d.with_help(format!(
                                    "change the value to match `{}`, or change the type annotation",
                                    self.ty_name(expected)
                                ))
                            };
                            self.diags.push(d);
                        }
                        expected
                    }
                    None => value_ty,
                };
                scopes.declare(
                    name.name.clone(),
                    VarInfo {
                        ty: var_ty,
                        mutable: *mutable,
                    },
                );
            }
            Stmt::Assign { target, value, .. } => {
                let target_ty = self.check_assign_target(target, scopes);
                let value_ty = self.check_expr(value, scopes);
                if let Some(target_ty) = target_ty {
                    if !compat(target_ty, value_ty) {
                        self.diags.push(
                            Diagnostic::error(
                                "E0012",
                                format!(
                                    "mismatched types: expected `{}`, found `{}`",
                                    self.ty_name(target_ty),
                                    self.ty_name(value_ty)
                                ),
                                value.span,
                            )
                            .with_label(format!("expected `{}`", self.ty_name(target_ty)))
                            .with_help("a variable keeps the type it was declared with"),
                        );
                    }
                }
            }
            Stmt::Expr(e) => {
                self.check_expr(e, scopes);
            }
            Stmt::Return { value, span } => match (value, ret) {
                (None, Ty::Unit) => {}
                (None, _) => {
                    if !ret.is_error() {
                        self.diags.push(
                            Diagnostic::error(
                                "E0012",
                                format!(
                                    "this function must return `{}`, but this `return` has no value",
                                    self.ty_name(ret)
                                ),
                                *span,
                            )
                            .with_help(format!("write `return <{}>;`", self.ty_name(ret))),
                        );
                    }
                }
                (Some(v), _) => {
                    let vty = self.check_expr(v, scopes);
                    if ret == Ty::Unit && !vty.is_error() {
                        self.diags.push(
                            Diagnostic::error(
                                "E0012",
                                format!(
                                    "this function has no return type, but this `return` has a value of type `{}`",
                                    self.ty_name(vty)
                                ),
                                v.span,
                            )
                            .with_help("declare a return type with `-> Type`, or drop the value"),
                        );
                    } else if !compat(ret, vty) {
                        self.diags.push(
                            Diagnostic::error(
                                "E0012",
                                format!(
                                    "mismatched types: expected `{}`, found `{}`",
                                    self.ty_name(ret),
                                    self.ty_name(vty)
                                ),
                                v.span,
                            )
                            .with_label(format!(
                                "expected `{}` because of the return type",
                                self.ty_name(ret)
                            )),
                        );
                    }
                }
            },
            Stmt::If(if_stmt) => self.check_if(if_stmt, scopes, ret),
            Stmt::While { cond, body, .. } => {
                self.check_condition(cond, scopes);
                self.check_block(body, scopes, ret);
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                let iter_ty = self.check_expr(iter, scopes);
                if !matches!(iter_ty, Ty::Range | Ty::Error) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0218",
                            format!(
                                "`for` loops iterate over Int ranges, found `{}`",
                                self.ty_name(iter_ty)
                            ),
                            iter.span,
                        )
                        .with_label("expected a range like `0..10`")
                        .with_note("iterating over collections is planned; ranges are half-open: `0..3` visits 0, 1, 2"),
                    );
                }
                scopes.push();
                scopes.declare(
                    var.name.clone(),
                    VarInfo {
                        ty: Ty::Int,
                        mutable: false,
                    },
                );
                self.check_block(body, scopes, ret);
                scopes.pop();
            }
            Stmt::Block(b) => self.check_block(b, scopes, ret),
        }
    }

    fn check_if(&mut self, if_stmt: &IfStmt, scopes: &mut Scopes, ret: Ty) {
        self.check_condition(&if_stmt.cond, scopes);
        self.check_block(&if_stmt.then_block, scopes, ret);
        match &if_stmt.else_branch {
            Some(ElseBranch::If(i)) => self.check_if(i, scopes, ret),
            Some(ElseBranch::Block(b)) => self.check_block(b, scopes, ret),
            None => {}
        }
    }

    fn check_condition(&mut self, cond: &Expr, scopes: &mut Scopes) {
        let ty = self.check_expr(cond, scopes);
        if !compat(ty, Ty::Bool) {
            self.diags.push(
                Diagnostic::error(
                    "E0211",
                    format!("this condition must be `Bool`, found `{}`", self.ty_name(ty)),
                    cond.span,
                )
                .with_label("expected `Bool`")
                .with_help(
                    "Kove has no implicit truthiness; write an explicit comparison such as `x != 0`",
                ),
            );
        }
    }

    /// Type of an assignment target, or None when a diagnostic was already
    /// reported for the target itself.
    fn check_assign_target(&mut self, target: &Expr, scopes: &mut Scopes) -> Option<Ty> {
        // Only a variable, or fields reached from a variable, can be
        // assigned. The whole chain's mutability comes from the root.
        match root_var(target) {
            Some(root_name) => {
                let Some(info) = scopes.lookup(root_name) else {
                    self.diags.push(
                        Diagnostic::error(
                            "E0201",
                            format!("cannot find variable `{}`", root_name),
                            target.span,
                        )
                        .with_label("not found in this scope")
                        .with_help("declare it first: `let ...`"),
                    );
                    return None;
                };
                if !info.mutable {
                    self.diags.push(
                        Diagnostic::error(
                            "E0204",
                            format!("cannot assign to `{}`: it is immutable", root_name),
                            target.span,
                        )
                        .with_label("this variable was not declared as mutable")
                        .with_help(format!(
                            "variables are immutable by default; declare it as `let mut {} = ...`",
                            root_name
                        )),
                    );
                    // Continue: the type check below is still useful.
                }
                // Re-derive the target's type through the field chain.
                Some(self.check_expr(target, scopes))
            }
            None => {
                self.diags.push(
                    Diagnostic::error("E0213", "invalid assignment target", target.span)
                        .with_label("cannot assign to this expression")
                        .with_help("only variables and their fields can be assigned, like `x = ...` or `user.age = ...`"),
                );
                None
            }
        }
    }

    fn check_expr(&mut self, e: &Expr, scopes: &mut Scopes) -> Ty {
        match &e.kind {
            ExprKind::Int(_) => Ty::Int,
            ExprKind::Float(_) => Ty::Float,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Char(_) => Ty::Char,
            ExprKind::Str(_) => Ty::Str,
            ExprKind::Error => Ty::Error,
            ExprKind::Var(name) => match scopes.lookup(name) {
                Some(info) => info.ty,
                None => {
                    let mut d = Diagnostic::error(
                        "E0201",
                        format!("cannot find variable `{}`", name),
                        e.span,
                    )
                    .with_label("not found in this scope");
                    if self.func_by_name.contains_key(name) {
                        d = d.with_help(format!(
                            "`{}` is a function; did you mean to call it: `{}(...)`?",
                            name, name
                        ));
                    } else if self.enum_by_name.contains_key(name) {
                        d = d.with_help(format!(
                            "`{}` is an enum; name one of its variants: `{}::...`",
                            name, name
                        ));
                    }
                    self.diags.push(d);
                    Ty::Error
                }
            },
            ExprKind::Unary { op, operand } => {
                let ty = self.check_expr(operand, scopes);
                let ok = match op {
                    UnaryOp::Neg => matches!(ty, Ty::Int | Ty::Float | Ty::Error),
                    UnaryOp::Not => matches!(ty, Ty::Bool | Ty::Error),
                };
                if !ok {
                    self.diags.push(
                        Diagnostic::error(
                            "E0212",
                            format!(
                                "cannot apply `{}` to a value of type `{}`",
                                op.symbol(),
                                self.ty_name(ty)
                            ),
                            e.span,
                        )
                        .with_label(match op {
                            UnaryOp::Neg => "`-` needs an Int or Float",
                            UnaryOp::Not => "`!` needs a Bool",
                        }),
                    );
                    return Ty::Error;
                }
                if ty.is_error() {
                    Ty::Error
                } else {
                    match op {
                        UnaryOp::Neg => ty,
                        UnaryOp::Not => Ty::Bool,
                    }
                }
            }
            ExprKind::Binary {
                op,
                lhs,
                rhs,
                op_span,
            } => self.check_binary(*op, lhs, rhs, *op_span, scopes),
            ExprKind::Range { lo, hi } => {
                for bound in [lo, hi] {
                    let ty = self.check_expr(bound, scopes);
                    if !compat(ty, Ty::Int) {
                        self.diags.push(
                            Diagnostic::error(
                                "E0012",
                                format!(
                                    "mismatched types: expected `Int`, found `{}`",
                                    self.ty_name(ty)
                                ),
                                bound.span,
                            )
                            .with_label("range bounds must be `Int`"),
                        );
                    }
                }
                Ty::Range
            }
            ExprKind::Call { callee, args } => self.check_call(e, callee, args, scopes),
            ExprKind::Field { base, name } => {
                let base_ty = self.check_expr(base, scopes);
                match base_ty {
                    Ty::Struct(idx) => {
                        match self.structs[idx]
                            .fields
                            .iter()
                            .find(|(n, _)| n == &name.name)
                        {
                            Some((_, ty)) => *ty,
                            None => {
                                let fields: Vec<&str> = self.structs[idx]
                                    .fields
                                    .iter()
                                    .map(|(n, _)| n.as_str())
                                    .collect();
                                self.diags.push(
                                    Diagnostic::error(
                                        "E0206",
                                        format!(
                                            "no field `{}` on struct `{}`",
                                            name.name, self.structs[idx].name
                                        ),
                                        name.span,
                                    )
                                    .with_label("unknown field")
                                    .with_help(
                                        if fields.is_empty() {
                                            format!("`{}` has no fields", self.structs[idx].name)
                                        } else {
                                            format!("available fields: {}", fields.join(", "))
                                        },
                                    ),
                                );
                                Ty::Error
                            }
                        }
                    }
                    Ty::Error => Ty::Error,
                    other => {
                        self.diags.push(
                            Diagnostic::error(
                                "E0209",
                                format!("a value of type `{}` has no fields", self.ty_name(other)),
                                e.span,
                            )
                            .with_label("field access needs a struct"),
                        );
                        Ty::Error
                    }
                }
            }
            ExprKind::StructLit { name, fields } => self.check_struct_lit(name, fields, scopes),
            ExprKind::Path { base, name } => {
                if let Some(&idx) = self.enum_by_name.get(&base.name) {
                    if self.enums[idx].variants.iter().any(|v| v == &name.name) {
                        Ty::Enum(idx)
                    } else {
                        let variants = self.enums[idx].variants.join(", ");
                        self.diags.push(
                            Diagnostic::error(
                                "E0216",
                                format!(
                                    "no variant `{}` on enum `{}`",
                                    name.name, self.enums[idx].name
                                ),
                                name.span,
                            )
                            .with_label("unknown variant")
                            .with_help(format!("available variants: {}", variants)),
                        );
                        Ty::Error
                    }
                } else if self.struct_by_name.contains_key(&base.name) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0216",
                            format!(
                                "`{}` is a struct, not an enum, so it has no variants",
                                base.name
                            ),
                            base.span,
                        )
                        .with_help(format!(
                            "construct it with a struct literal: `{} {{ ... }}`",
                            base.name
                        )),
                    );
                    Ty::Error
                } else {
                    self.diags.push(
                        Diagnostic::error(
                            "E0200",
                            format!("cannot find type `{}`", base.name),
                            base.span,
                        )
                        .with_label("not a known enum"),
                    );
                    Ty::Error
                }
            }
        }
    }

    fn check_binary(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        op_span: Span,
        scopes: &mut Scopes,
    ) -> Ty {
        let lt = self.check_expr(lhs, scopes);
        let rt = self.check_expr(rhs, scopes);
        if lt.is_error() || rt.is_error() {
            return Ty::Error;
        }
        let invalid = |c: &mut Checker, label: &str| {
            let mut d = Diagnostic::error(
                "E0212",
                format!(
                    "cannot apply `{}` to `{}` and `{}`",
                    op.symbol(),
                    c.ty_name(lt),
                    c.ty_name(rt)
                ),
                op_span,
            )
            .with_label(label.to_string());
            if matches!((lt, rt), (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int)) {
                d = d.with_note(
                    "Kove never converts between Int and Float implicitly; \
                     make both sides the same type",
                );
            }
            c.diags.push(d);
            Ty::Error
        };
        use BinaryOp::*;
        match op {
            Add | Sub | Mul | Div | Rem => match (lt, rt) {
                (Ty::Int, Ty::Int) => Ty::Int,
                (Ty::Float, Ty::Float) => Ty::Float,
                _ => invalid(self, "arithmetic needs two Ints or two Floats"),
            },
            Lt | Le | Gt | Ge => match (lt, rt) {
                (Ty::Int, Ty::Int) | (Ty::Float, Ty::Float) => Ty::Bool,
                _ => invalid(self, "ordering needs two Ints or two Floats"),
            },
            Eq | Ne => {
                let comparable = matches!(
                    lt,
                    Ty::Int | Ty::Float | Ty::Bool | Ty::Char | Ty::Str | Ty::Enum(_)
                );
                if comparable && lt == rt {
                    Ty::Bool
                } else {
                    invalid(
                        self,
                        "equality needs two values of the same comparable type",
                    )
                }
            }
            And | Or => match (lt, rt) {
                (Ty::Bool, Ty::Bool) => Ty::Bool,
                _ => invalid(self, "logical operators need two Bools"),
            },
        }
    }

    fn check_call(&mut self, call: &Expr, callee: &Expr, args: &[Expr], scopes: &mut Scopes) -> Ty {
        let ExprKind::Var(name) = &callee.kind else {
            if !matches!(callee.kind, ExprKind::Error) {
                self.diags.push(
                    Diagnostic::error("E0230", "only named functions can be called", callee.span)
                        .with_label("not a function name")
                        .with_note("methods and function values are not part of the language yet"),
                );
            }
            for a in args {
                self.check_expr(a, scopes);
            }
            return Ty::Error;
        };

        if name == "println" {
            if args.len() != 1 {
                self.diags.push(
                    Diagnostic::error(
                        "E0203",
                        format!(
                            "`println` takes exactly 1 argument, but {} were supplied",
                            args.len()
                        ),
                        call.span,
                    )
                    .with_label("expected 1 argument"),
                );
            }
            for a in args {
                let ty = self.check_expr(a, scopes);
                if !matches!(
                    ty,
                    Ty::Int | Ty::Float | Ty::Bool | Ty::Char | Ty::Str | Ty::Error
                ) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0215",
                            format!("cannot print a value of type `{}`", self.ty_name(ty)),
                            a.span,
                        )
                        .with_help("`println` accepts Int, Float, Bool, Char and String"),
                    );
                }
            }
            return Ty::Unit;
        }

        let Some(&idx) = self.func_by_name.get(name) else {
            let mut d = Diagnostic::error(
                "E0202",
                format!("cannot find function `{}`", name),
                callee.span,
            )
            .with_label("not defined anywhere in this file");
            if scopes.lookup(name).is_some() {
                d = d.with_help(format!("`{}` is a variable, not a function", name));
            }
            self.diags.push(d);
            for a in args {
                self.check_expr(a, scopes);
            }
            return Ty::Error;
        };

        let (params, ret) = (self.funcs[idx].params.clone(), self.funcs[idx].ret);
        if args.len() != params.len() {
            self.diags.push(
                Diagnostic::error(
                    "E0203",
                    format!(
                        "`{}` takes {} argument{}, but {} {} supplied",
                        name,
                        params.len(),
                        if params.len() == 1 { "" } else { "s" },
                        args.len(),
                        if args.len() == 1 { "was" } else { "were" }
                    ),
                    call.span,
                )
                .with_label(format!(
                    "expected {} argument{}",
                    params.len(),
                    if params.len() == 1 { "" } else { "s" }
                ))
                .with_note(format!(
                    "`{}` is declared as `fn {}({})`",
                    name,
                    name,
                    params
                        .iter()
                        .map(|(pn, pt)| format!("{}: {}", pn, self.ty_name(*pt)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
            // Still check the argument expressions themselves.
            for a in args {
                self.check_expr(a, scopes);
            }
            return ret;
        }
        for (a, (pname, pty)) in args.iter().zip(params.iter()) {
            let aty = self.check_expr(a, scopes);
            if !compat(aty, *pty) {
                self.diags.push(
                    Diagnostic::error(
                        "E0012",
                        format!(
                            "mismatched types: expected `{}`, found `{}`",
                            self.ty_name(*pty),
                            self.ty_name(aty)
                        ),
                        a.span,
                    )
                    .with_label(format!("expected `{}`", self.ty_name(*pty)))
                    .with_note(format!(
                        "the parameter `{}` of `{}` is declared as `{}`",
                        pname,
                        name,
                        self.ty_name(*pty)
                    )),
                );
            }
        }
        ret
    }

    fn check_struct_lit(
        &mut self,
        name: &Ident,
        fields: &[(Ident, Expr)],
        scopes: &mut Scopes,
    ) -> Ty {
        let Some(&idx) = self.struct_by_name.get(&name.name) else {
            if self.enum_by_name.contains_key(&name.name) {
                self.diags.push(
                    Diagnostic::error(
                        "E0219",
                        format!("`{}` is an enum, not a struct", name.name),
                        name.span,
                    )
                    .with_help(format!(
                        "name one of its variants instead: `{}::...`",
                        name.name
                    )),
                );
            } else {
                self.diags.push(
                    Diagnostic::error(
                        "E0200",
                        format!("cannot find type `{}`", name.name),
                        name.span,
                    )
                    .with_label("not a known struct"),
                );
            }
            for (_, value) in fields {
                self.check_expr(value, scopes);
            }
            return Ty::Error;
        };

        let declared = self.structs[idx].fields.clone();
        let struct_name = self.structs[idx].name.clone();
        let mut seen: Vec<&str> = Vec::new();
        for (fname, value) in fields {
            let vty = self.check_expr(value, scopes);
            if seen.contains(&fname.name.as_str()) {
                self.diags.push(
                    Diagnostic::error(
                        "E0208",
                        format!("the field `{}` is initialized more than once", fname.name),
                        fname.span,
                    )
                    .with_label("duplicate field"),
                );
                continue;
            }
            seen.push(&fname.name);
            match declared.iter().find(|(n, _)| n == &fname.name) {
                Some((_, fty)) => {
                    if !compat(vty, *fty) {
                        self.diags.push(
                            Diagnostic::error(
                                "E0012",
                                format!(
                                    "mismatched types: expected `{}`, found `{}`",
                                    self.ty_name(*fty),
                                    self.ty_name(vty)
                                ),
                                value.span,
                            )
                            .with_label(format!("expected `{}`", self.ty_name(*fty)))
                            .with_note(format!(
                                "the field `{}` of `{}` is declared as `{}`",
                                fname.name,
                                struct_name,
                                self.ty_name(*fty)
                            )),
                        );
                    }
                }
                None => {
                    self.diags.push(
                        Diagnostic::error(
                            "E0206",
                            format!("no field `{}` on struct `{}`", fname.name, struct_name),
                            fname.span,
                        )
                        .with_label("unknown field"),
                    );
                }
            }
        }
        let missing: Vec<&str> = declared
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| !seen.contains(n))
            .collect();
        if !missing.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    "E0207",
                    format!(
                        "missing field{} {} in the initializer of `{}`",
                        if missing.len() == 1 { "" } else { "s" },
                        missing
                            .iter()
                            .map(|n| format!("`{}`", n))
                            .collect::<Vec<_>>()
                            .join(", "),
                        struct_name
                    ),
                    name.span,
                )
                .with_label("every field must be given a value"),
            );
        }
        Ty::Struct(idx)
    }
}

// --- Helpers ---------------------------------------------------------------

struct Scopes {
    scopes: Vec<HashMap<String, VarInfo>>,
}

impl Scopes {
    fn new() -> Scopes {
        Scopes { scopes: Vec::new() }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: String, info: VarInfo) {
        self.scopes
            .last_mut()
            .expect("scope stack is never empty while checking")
            .insert(name, info);
    }

    fn lookup(&self, name: &str) -> Option<VarInfo> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).copied()
    }
}

/// The variable at the root of an assignment target's field chain.
fn root_var(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Var(name) => Some(name),
        ExprKind::Field { base, .. } => root_var(base),
        _ => None,
    }
}

/// Conservative "every path returns" analysis: true when the block contains
/// a `return`, or an `if`/`else` whose branches all return, or a nested
/// block that does. Loops never count, since their bodies may not run.
fn always_returns(block: &Block) -> bool {
    block.stmts.iter().any(stmt_always_returns)
}

fn stmt_always_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } => true,
        Stmt::Block(b) => always_returns(b),
        Stmt::If(i) => if_always_returns(i),
        _ => false,
    }
}

fn if_always_returns(i: &IfStmt) -> bool {
    if !always_returns(&i.then_block) {
        return false;
    }
    match &i.else_branch {
        Some(ElseBranch::Block(b)) => always_returns(b),
        Some(ElseBranch::If(next)) => if_always_returns(next),
        None => false,
    }
}
