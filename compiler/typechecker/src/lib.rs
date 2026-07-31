//! Type checking.
//!
//! Runs after `kove-resolver`, and deliberately knows nothing about
//! scopes or lookup: every name in the program has already been bound to
//! a local, a function or an item, so this stage only assigns and compares
//! types. Where the resolver produced a [`LocalId`], the checker records
//! that binding's type; where the resolver recorded a reference, the
//! checker reads the type straight back.
//!
//! Error handling philosophy: a diagnostic is reported once at the point
//! where checking fails and the offending expression gets [`Ty::Error`],
//! which is compatible with everything, so one mistake produces one error
//! instead of a cascade, and the checker keeps going to report every
//! independent error in the file. Names the resolver could not resolve
//! arrive as `Error` too, which is why an unknown variable does not also
//! produce a type error.

use kove_ast::*;
use kove_diagnostics::{Diagnostic, Span};
use kove_resolver::{Builtin, EnumId, LocalId, Resolution, Resolutions, StructId, TypeRef};
use std::collections::HashMap;

/// A Kove type. Everything a [`TypeRef`] can be, plus the types nobody
/// writes down: the unit type of a function with no return type, and the
/// range produced by `lo..hi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    Char,
    Str,
    Unit,
    Struct(StructId),
    Enum(EnumId),
    /// The type of `lo..hi`; only a `for` loop can consume it.
    Range,
    /// An expression that already failed to check; compatible with anything.
    Error,
}

impl Ty {
    fn of(t: TypeRef) -> Ty {
        match t {
            TypeRef::Int => Ty::Int,
            TypeRef::Float => Ty::Float,
            TypeRef::Bool => Ty::Bool,
            TypeRef::Char => Ty::Char,
            TypeRef::Str => Ty::Str,
            TypeRef::Struct(id) => Ty::Struct(id),
            TypeRef::Enum(id) => Ty::Enum(id),
            TypeRef::Error => Ty::Error,
        }
    }

    fn is_error(self) -> bool {
        matches!(self, Ty::Error)
    }
}

/// Types are compatible if equal or if either side already failed.
fn compat(a: Ty, b: Ty) -> bool {
    a == b || a.is_error() || b.is_error()
}

/// Type-check a resolved program.
pub fn check(program: &Program, res: &Resolutions) -> Vec<Diagnostic> {
    let mut c = Checker {
        res,
        locals: HashMap::new(),
        diags: Vec::new(),
    };
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

struct Checker<'r> {
    res: &'r Resolutions,
    /// The type of every binding the resolver created.
    locals: HashMap<LocalId, Ty>,
    diags: Vec<Diagnostic>,
}

impl<'r> Checker<'r> {
    fn ty_name(&self, ty: Ty) -> String {
        match ty {
            Ty::Int => "Int".into(),
            Ty::Float => "Float".into(),
            Ty::Bool => "Bool".into(),
            Ty::Char => "Char".into(),
            Ty::Str => "String".into(),
            Ty::Unit => "()".into(),
            Ty::Struct(id) => self.res.struct_def(id).name.clone(),
            Ty::Enum(id) => self.res.enum_def(id).name.clone(),
            Ty::Range => "Range".into(),
            Ty::Error => "<error>".into(),
        }
    }

    /// The type of a binding, or `Error` if it never got one (which only
    /// happens when its declaration already failed).
    fn local_ty(&self, local: LocalId) -> Ty {
        self.locals.get(&local).copied().unwrap_or(Ty::Error)
    }

    fn bind(&mut self, ident: &Ident, ty: Ty) {
        if let Some(local) = self.res.binding(ident.id) {
            self.locals.insert(local, ty);
        }
    }

    fn check_function(&mut self, f: &Function) {
        let Some(id) = self.res.func_of_decl(f.name.id) else {
            return; // a duplicate definition; the first one was checked
        };
        let def = self.res.func_def(id);
        let declared_ret = def.ret.map(Ty::of);
        for p in &def.params {
            self.locals.insert(p.local, Ty::of(p.ty));
        }

        self.check_block(&f.body, declared_ret);

        if let Some(ret) = declared_ret {
            if !ret.is_error() && !always_returns(&f.body) {
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
    }

    fn check_block(&mut self, block: &Block, ret: Option<Ty>) {
        for stmt in &block.stmts {
            self.check_stmt(stmt, ret);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, ret: Option<Ty>) {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let value_ty = self.check_expr(value);
                let var_ty = match ty {
                    Some(ann) => {
                        let expected = Ty::of(self.res.type_ref(ann));
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
                self.bind(name, var_ty);
            }
            Stmt::Assign { target, value, .. } => {
                // Whether the target *may* be assigned is the resolver's
                // call; here we only check that the types line up.
                let target_ty = self.check_expr(target);
                let value_ty = self.check_expr(value);
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
            Stmt::Expr(e) => {
                self.check_expr(e);
            }
            Stmt::Return { value, span } => self.check_return(value.as_ref(), *span, ret),
            Stmt::If(if_stmt) => self.check_if(if_stmt, ret),
            Stmt::While { cond, body, .. } => {
                self.check_condition(cond);
                self.check_block(body, ret);
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                let iter_ty = self.check_expr(iter);
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
                        .with_note(
                            "iterating over collections is planned; ranges are half-open: \
                             `0..3` visits 0, 1, 2",
                        ),
                    );
                }
                self.bind(var, Ty::Int);
                self.check_block(body, ret);
            }
            Stmt::Block(b) => self.check_block(b, ret),
        }
    }

    fn check_return(&mut self, value: Option<&Expr>, span: Span, ret: Option<Ty>) {
        let declared = ret.unwrap_or(Ty::Unit);
        match value {
            None => {
                if ret.is_some() && !declared.is_error() {
                    self.diags.push(
                        Diagnostic::error(
                            "E0012",
                            format!(
                                "this function must return `{}`, but this `return` has no value",
                                self.ty_name(declared)
                            ),
                            span,
                        )
                        .with_help(format!("write `return <{}>;`", self.ty_name(declared))),
                    );
                }
            }
            Some(v) => {
                let vty = self.check_expr(v);
                if ret.is_none() && !vty.is_error() {
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
                } else if !compat(declared, vty) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0012",
                            format!(
                                "mismatched types: expected `{}`, found `{}`",
                                self.ty_name(declared),
                                self.ty_name(vty)
                            ),
                            v.span,
                        )
                        .with_label(format!(
                            "expected `{}` because of the return type",
                            self.ty_name(declared)
                        )),
                    );
                }
            }
        }
    }

    fn check_if(&mut self, if_stmt: &IfStmt, ret: Option<Ty>) {
        self.check_condition(&if_stmt.cond);
        self.check_block(&if_stmt.then_block, ret);
        match &if_stmt.else_branch {
            Some(ElseBranch::If(i)) => self.check_if(i, ret),
            Some(ElseBranch::Block(b)) => self.check_block(b, ret),
            None => {}
        }
    }

    fn check_condition(&mut self, cond: &Expr) {
        let ty = self.check_expr(cond);
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

    fn check_expr(&mut self, e: &Expr) -> Ty {
        match &e.kind {
            ExprKind::Int(_) => Ty::Int,
            ExprKind::Float(_) => Ty::Float,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Char(_) => Ty::Char,
            ExprKind::Str(_) => Ty::Str,
            ExprKind::Error => Ty::Error,
            ExprKind::Var(_) => match self.res.resolution(e.id) {
                Resolution::Local(local) => self.local_ty(local),
                // Anything else was already reported by the resolver.
                _ => Ty::Error,
            },
            ExprKind::Path { .. } => match self.res.resolution(e.id) {
                Resolution::Variant(id) => Ty::Enum(id),
                _ => Ty::Error,
            },
            ExprKind::Unary { op, operand } => {
                let ty = self.check_expr(operand);
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
            } => self.check_binary(*op, lhs, rhs, *op_span),
            ExprKind::Range { lo, hi } => {
                for bound in [lo, hi] {
                    let ty = self.check_expr(bound);
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
            ExprKind::Call { callee, args } => self.check_call(e, callee, args),
            ExprKind::Field { base, name } => {
                let base_ty = self.check_expr(base);
                match base_ty {
                    Ty::Struct(id) => {
                        let def = self.res.struct_def(id);
                        match def.fields.iter().find(|(n, _)| n == &name.name) {
                            Some((_, ty)) => Ty::of(*ty),
                            None => {
                                let fields: Vec<&str> =
                                    def.fields.iter().map(|(n, _)| n.as_str()).collect();
                                let help = if fields.is_empty() {
                                    format!("`{}` has no fields", def.name)
                                } else {
                                    format!("available fields: {}", fields.join(", "))
                                };
                                let message =
                                    format!("no field `{}` on struct `{}`", name.name, def.name);
                                self.diags.push(
                                    Diagnostic::error("E0206", message, name.span)
                                        .with_label("unknown field")
                                        .with_help(help),
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
            ExprKind::StructLit { name, fields } => self.check_struct_lit(name, fields),
        }
    }

    fn check_binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, op_span: Span) -> Ty {
        let lt = self.check_expr(lhs);
        let rt = self.check_expr(rhs);
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

    fn check_call(&mut self, call: &Expr, callee: &Expr, args: &[Expr]) -> Ty {
        match self.res.resolution(callee.id) {
            Resolution::Builtin(Builtin::Assert) => {
                self.check_builtin_arity(call, args, Builtin::Assert, 1);
                for a in args {
                    let ty = self.check_expr(a);
                    if !compat(ty, Ty::Bool) {
                        self.diags.push(
                            Diagnostic::error(
                                "E0211",
                                format!("`assert` needs a `Bool`, found `{}`", self.ty_name(ty)),
                                a.span,
                            )
                            .with_label("expected `Bool`")
                            .with_help("write a comparison, such as `assert(x == 1)`"),
                        );
                    }
                }
                Ty::Unit
            }
            Resolution::Builtin(builtin @ (Builtin::ToFloat | Builtin::ToInt)) => {
                let (from, to) = match builtin {
                    Builtin::ToFloat => (Ty::Int, Ty::Float),
                    _ => (Ty::Float, Ty::Int),
                };
                self.check_builtin_arity(call, args, builtin, 1);
                for a in args {
                    let ty = self.check_expr(a);
                    if !compat(ty, from) {
                        self.diags.push(
                            Diagnostic::error(
                                "E0012",
                                format!(
                                    "mismatched types: expected `{}`, found `{}`",
                                    self.ty_name(from),
                                    self.ty_name(ty)
                                ),
                                a.span,
                            )
                            .with_label(format!("expected `{}`", self.ty_name(from)))
                            .with_note(format!(
                                "`{}` converts `{}` to `{}`",
                                builtin.name(),
                                self.ty_name(from),
                                self.ty_name(to)
                            )),
                        );
                    }
                }
                to
            }
            Resolution::Builtin(Builtin::Println) => {
                self.check_builtin_arity(call, args, Builtin::Println, 1);
                for a in args {
                    let ty = self.check_expr(a);
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
                Ty::Unit
            }
            Resolution::Function(id) => {
                let def = self.res.func_def(id);
                let name = def.name.clone();
                let params: Vec<(String, Ty)> = def
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), Ty::of(p.ty)))
                    .collect();
                let ret = def.ret.map(Ty::of).unwrap_or(Ty::Unit);

                if args.len() != params.len() {
                    let signature = params
                        .iter()
                        .map(|(pn, pt)| format!("{}: {}", pn, self.ty_name(*pt)))
                        .collect::<Vec<_>>()
                        .join(", ");
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
                            name, name, signature
                        )),
                    );
                    for a in args {
                        self.check_expr(a);
                    }
                    return ret;
                }

                for (a, (pname, pty)) in args.iter().zip(params.iter()) {
                    let aty = self.check_expr(a);
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
            // Unresolvable callee; the resolver reported it.
            _ => {
                for a in args {
                    self.check_expr(a);
                }
                Ty::Error
            }
        }
    }

    fn check_builtin_arity(&mut self, call: &Expr, args: &[Expr], builtin: Builtin, want: usize) {
        if args.len() == want {
            return;
        }
        self.diags.push(
            Diagnostic::error(
                "E0203",
                format!(
                    "`{}` takes exactly {} argument{}, but {} {} supplied",
                    builtin.name(),
                    want,
                    if want == 1 { "" } else { "s" },
                    args.len(),
                    if args.len() == 1 { "was" } else { "were" }
                ),
                call.span,
            )
            .with_label(format!(
                "expected {} argument{}",
                want,
                if want == 1 { "" } else { "s" }
            )),
        );
    }

    fn check_struct_lit(&mut self, name: &Ident, fields: &[(Ident, Expr)]) -> Ty {
        let Resolution::Struct(id) = self.res.resolution(name.id) else {
            // Unknown struct name; the resolver reported it. Still check
            // the field values so mistakes inside them are not hidden.
            for (_, value) in fields {
                self.check_expr(value);
            }
            return Ty::Error;
        };

        let def = self.res.struct_def(id);
        let struct_name = def.name.clone();
        let declared: Vec<(String, Ty)> = def
            .fields
            .iter()
            .map(|(n, t)| (n.clone(), Ty::of(*t)))
            .collect();

        let mut seen: Vec<&str> = Vec::new();
        for (fname, value) in fields {
            let vty = self.check_expr(value);
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
        Ty::Struct(id)
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
