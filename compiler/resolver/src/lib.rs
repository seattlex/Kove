//! Name resolution: what does each name in the program refer to?
//!
//! This runs before type checking and answers a different question. The
//! resolver knows that `count` in a body refers to the `let` three lines
//! up, and that the `let` was not declared `mut`; it does not know or care
//! that `count` is an `Int`. The type checker knows the second thing and
//! never has to look a name up.
//!
//! Two passes:
//!
//! 1. Items. Every struct, enum and function gets an id and a signature,
//!    so items can refer to each other in any order.
//! 2. Bodies. A lexical scope stack binds parameters, `let` declarations
//!    and `for` variables to [`LocalId`]s, and every reference is recorded
//!    in the [`Resolutions`] map keyed by [`NodeId`].
//!
//! Diagnostics owned here are the ones about names: unknown types (E0200),
//! variables (E0201) and functions (E0202), assignment to something
//! immutable (E0204) or unassignable (E0213), duplicate definitions
//! (E0205), unknown enum variants (E0216), unsupported imports (E0217),
//! struct-literal syntax on an enum (E0219), and calls to things that are
//! not named functions (E0230). Anything about *types* belongs to
//! `kove-typechecker`.

mod suggest;

use kove_ast::*;
use kove_diagnostics::{Diagnostic, Span};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumId(pub usize);

/// A type as *written* in source, resolved to what it names. The type
/// checker builds its own richer `Ty` on top of this (adding things no
/// one writes down yet, like the type of a range).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRef {
    Int,
    Float,
    Bool,
    Char,
    Str,
    Struct(StructId),
    Enum(EnumId),
    /// Unknown or malformed; a diagnostic was already reported.
    Error,
}

/// What a name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Local(LocalId),
    Function(FuncId),
    Builtin(Builtin),
    /// An `Enum::Variant` path.
    Variant(EnumId),
    /// The name of a struct in a struct literal.
    Struct(StructId),
    /// Unresolved; a diagnostic was already reported and downstream
    /// stages should stay quiet about this name.
    Error,
}

/// Functions the language provides without declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Println,
    Assert,
}

impl Builtin {
    pub const ALL: [Builtin; 2] = [Builtin::Println, Builtin::Assert];

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Println => "println",
            Builtin::Assert => "assert",
        }
    }

    pub fn from_name(name: &str) -> Option<Builtin> {
        Builtin::ALL.into_iter().find(|b| b.name() == name)
    }
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub span: Span,
    pub fields: Vec<(String, TypeRef)>,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub span: Span,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<ParamDef>,
    /// `None` when the function declares no return type.
    pub ret: Option<TypeRef>,
}

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: String,
    pub ty: TypeRef,
    /// The binding the parameter introduces, so the type checker can give
    /// it a type without re-walking scopes.
    pub local: LocalId,
}

#[derive(Debug, Clone)]
pub struct LocalDef {
    pub name: String,
    pub mutable: bool,
}

/// Everything the resolver learned, consumed by later stages.
#[derive(Debug, Default)]
pub struct Resolutions {
    structs: Vec<StructDef>,
    enums: Vec<EnumDef>,
    funcs: Vec<FuncDef>,
    locals: Vec<LocalDef>,
    struct_by_name: HashMap<String, StructId>,
    enum_by_name: HashMap<String, EnumId>,
    func_by_name: HashMap<String, FuncId>,
    /// The declaring identifier of each accepted function definition. A
    /// duplicate definition is absent, which is how later stages skip it
    /// instead of checking it against the first definition's signature.
    func_decls: HashMap<NodeId, FuncId>,
    /// References: a `Var` or `Path` expression's id, or the name `Ident`
    /// of a struct literal, mapped to what it names.
    refs: HashMap<NodeId, Resolution>,
    /// Declarations: a declaring `Ident`'s id mapped to the binding it
    /// introduces.
    bindings: HashMap<NodeId, LocalId>,
}

impl Resolutions {
    pub fn struct_def(&self, id: StructId) -> &StructDef {
        &self.structs[id.0]
    }

    pub fn enum_def(&self, id: EnumId) -> &EnumDef {
        &self.enums[id.0]
    }

    pub fn func_def(&self, id: FuncId) -> &FuncDef {
        &self.funcs[id.0]
    }

    pub fn local(&self, id: LocalId) -> &LocalDef {
        &self.locals[id.0 as usize]
    }

    pub fn func_id(&self, name: &str) -> Option<FuncId> {
        self.func_by_name.get(name).copied()
    }

    /// The function a declaration introduced, keyed by its name's id.
    /// `None` means the declaration was rejected as a duplicate.
    pub fn func_of_decl(&self, name_id: NodeId) -> Option<FuncId> {
        self.func_decls.get(&name_id).copied()
    }

    /// What a reference names. Unrecorded ids resolve to
    /// [`Resolution::Error`], which is also what unresolvable names get,
    /// so callers never need to special-case a missing entry.
    pub fn resolution(&self, id: NodeId) -> Resolution {
        self.refs.get(&id).copied().unwrap_or(Resolution::Error)
    }

    /// The binding a declaring identifier introduced.
    pub fn binding(&self, id: NodeId) -> Option<LocalId> {
        self.bindings.get(&id).copied()
    }

    /// Look up a written type without reporting anything. Unknown names
    /// were already reported during resolution.
    pub fn type_ref(&self, t: &TypeExpr) -> TypeRef {
        match t.name.as_str() {
            "Int" => TypeRef::Int,
            "Float" => TypeRef::Float,
            "Bool" => TypeRef::Bool,
            "Char" => TypeRef::Char,
            "String" => TypeRef::Str,
            name => {
                if let Some(&i) = self.struct_by_name.get(name) {
                    TypeRef::Struct(i)
                } else if let Some(&i) = self.enum_by_name.get(name) {
                    TypeRef::Enum(i)
                } else {
                    TypeRef::Error
                }
            }
        }
    }
}

/// Resolve every name in a program.
pub fn resolve(program: &Program) -> (Resolutions, Vec<Diagnostic>) {
    let mut r = Resolver::default();
    r.collect_items(program);
    for item in &program.items {
        if let Item::Function(f) = item {
            r.resolve_function(f);
        }
    }
    r.diags.sort_by_key(|d| (d.span.start, d.span.end));
    (r.out, r.diags)
}

#[derive(Default)]
struct Resolver {
    out: Resolutions,
    scopes: Vec<HashMap<String, LocalId>>,
    diags: Vec<Diagnostic>,
}

impl Resolver {
    // --- Pass 1: items ----------------------------------------------------

    fn collect_items(&mut self, program: &Program) {
        // Names first, so signatures can mention any item.
        for item in &program.items {
            match item {
                Item::Struct(s) => {
                    if self.duplicate_type(&s.name) {
                        continue;
                    }
                    let id = StructId(self.out.structs.len());
                    self.out.structs.push(StructDef {
                        name: s.name.name.clone(),
                        span: s.name.span,
                        fields: Vec::new(),
                    });
                    self.out.struct_by_name.insert(s.name.name.clone(), id);
                }
                Item::Enum(e) => {
                    if self.duplicate_type(&e.name) {
                        continue;
                    }
                    let id = EnumId(self.out.enums.len());
                    self.out.enums.push(EnumDef {
                        name: e.name.name.clone(),
                        span: e.name.span,
                        variants: e.variants.iter().map(|v| v.name.clone()).collect(),
                    });
                    self.out.enum_by_name.insert(e.name.name.clone(), id);
                }
                Item::Function(f) => {
                    if Builtin::from_name(&f.name.name).is_some() {
                        self.diags.push(
                            Diagnostic::error(
                                "E0205",
                                format!(
                                    "the name `{}` is reserved for a built-in function",
                                    f.name.name
                                ),
                                f.name.span,
                            )
                            .with_label("cannot be redefined"),
                        );
                        continue;
                    }
                    if self.out.func_by_name.contains_key(&f.name.name) {
                        self.diags.push(
                            Diagnostic::error(
                                "E0205",
                                format!("the function `{}` is defined more than once", f.name.name),
                                f.name.span,
                            )
                            .with_label("redefined here")
                            .with_help("give each function a unique name"),
                        );
                        continue;
                    }
                    let id = FuncId(self.out.funcs.len());
                    self.out.funcs.push(FuncDef {
                        name: f.name.name.clone(),
                        params: Vec::new(),
                        ret: None,
                    });
                    self.out.func_by_name.insert(f.name.name.clone(), id);
                    self.out.func_decls.insert(f.name.id, id);
                }
                Item::Import(imp) => {
                    let path: Vec<&str> = imp.path.iter().map(|i| i.name.as_str()).collect();
                    self.diags.push(
                        Diagnostic::error(
                            "E0217",
                            format!(
                                "the module system is not implemented yet (`import {}`)",
                                path.join("::")
                            ),
                            imp.span,
                        )
                        .with_note(
                            "modules are planned (see docs/language.md); for now a program is a single file",
                        ),
                    );
                }
            }
        }

        // Signatures second.
        for item in &program.items {
            match item {
                Item::Struct(s) => {
                    let Some(&id) = self.out.struct_by_name.get(&s.name.name) else {
                        continue;
                    };
                    let mut fields: Vec<(String, TypeRef)> = Vec::new();
                    for f in &s.fields {
                        if fields.iter().any(|(n, _)| n == &f.name.name) {
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
                    self.out.structs[id.0].fields = fields;
                }
                Item::Enum(e) => {
                    let mut seen: Vec<&str> = Vec::new();
                    for v in &e.variants {
                        if seen.contains(&v.name.as_str()) {
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
                        seen.push(&v.name);
                    }
                }
                Item::Function(f) => {
                    // Keyed by declaration, so a rejected duplicate never
                    // overwrites the signature of the definition that won.
                    let Some(id) = self.out.func_of_decl(f.name.id) else {
                        continue;
                    };
                    let mut params: Vec<ParamDef> = Vec::new();
                    for p in &f.params {
                        if params.iter().any(|q| q.name == p.name.name) {
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
                        let local = self.new_local(&p.name, false);
                        params.push(ParamDef {
                            name: p.name.name.clone(),
                            ty,
                            local,
                        });
                    }
                    let ret = f.return_type.as_ref().map(|t| self.resolve_type(t));
                    self.out.funcs[id.0].params = params;
                    self.out.funcs[id.0].ret = ret;
                }
                Item::Import(_) => {}
            }
        }
    }

    /// Reports E0205 and returns true when a type name is already taken.
    fn duplicate_type(&mut self, name: &Ident) -> bool {
        let taken = self.out.struct_by_name.contains_key(&name.name)
            || self.out.enum_by_name.contains_key(&name.name);
        if taken {
            self.diags.push(
                Diagnostic::error(
                    "E0205",
                    format!("the type `{}` is defined more than once", name.name),
                    name.span,
                )
                .with_label("redefined here")
                .with_help("give each struct and enum a unique name"),
            );
        }
        taken
    }

    fn resolve_type(&mut self, t: &TypeExpr) -> TypeRef {
        // A malformed type annotation already produced a syntax error.
        if t.name == "<error>" {
            return TypeRef::Error;
        }
        let resolved = self.out.type_ref(t);
        if resolved == TypeRef::Error {
            let mut d =
                Diagnostic::error("E0200", format!("cannot find type `{}`", t.name), t.span)
                    .with_label("not a known type");
            d = match self.closest_type(&t.name) {
                Some(name) => d.with_help(format!("did you mean `{}`?", name)),
                None => d.with_help(
                    "the primitive types are Int, Float, Bool, Char and String; \
                     structs and enums must be declared in this file",
                ),
            };
            self.diags.push(d);
        }
        resolved
    }

    /// Type names in scope: the primitives plus every declared struct and
    /// enum.
    fn closest_type(&self, name: &str) -> Option<String> {
        let declared: Vec<&str> = self
            .out
            .struct_by_name
            .keys()
            .chain(self.out.enum_by_name.keys())
            .map(String::as_str)
            .collect();
        let candidates = ["Int", "Float", "Bool", "Char", "String"]
            .into_iter()
            .chain(declared);
        suggest::closest(name, candidates).map(str::to_string)
    }

    /// Every variable currently visible, innermost scope first.
    fn visible_locals(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for scope in self.scopes.iter().rev() {
            for name in scope.keys() {
                out.push(name.as_str());
            }
        }
        out
    }

    // --- Pass 2: bodies ---------------------------------------------------

    fn new_local(&mut self, name: &Ident, mutable: bool) -> LocalId {
        let id = LocalId(self.out.locals.len() as u32);
        self.out.locals.push(LocalDef {
            name: name.name.clone(),
            mutable,
        });
        self.out.bindings.insert(name.id, id);
        id
    }

    /// Introduce a binding into the innermost scope.
    fn declare(&mut self, name: &Ident, mutable: bool) {
        let id = self.new_local(name, mutable);
        self.scopes
            .last_mut()
            .expect("a scope is open while resolving a body")
            .insert(name.name.clone(), id);
    }

    fn lookup(&self, name: &str) -> Option<LocalId> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).copied()
    }

    fn resolve_function(&mut self, f: &Function) {
        let Some(id) = self.out.func_of_decl(f.name.id) else {
            return; // a duplicate definition; the first one owns the name
        };
        self.scopes.push(HashMap::new());
        // Parameters were given ids during the signature pass; bring those
        // same bindings into scope rather than creating new ones.
        let params: Vec<(String, LocalId)> = self.out.funcs[id.0]
            .params
            .iter()
            .map(|p| (p.name.clone(), p.local))
            .collect();
        for (name, local) in params {
            self.scopes
                .last_mut()
                .expect("parameter scope is open")
                .insert(name, local);
        }
        self.resolve_block(&f.body);
        self.scopes.pop();
    }

    fn resolve_block(&mut self, block: &Block) {
        self.scopes.push(HashMap::new());
        for stmt in &block.stmts {
            self.resolve_stmt(stmt);
        }
        self.scopes.pop();
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                mutable,
                name,
                ty,
                value,
                ..
            } => {
                // The initializer is resolved first, so `let x = x;` refers
                // to an outer `x` rather than to itself.
                self.resolve_expr(value);
                if let Some(ty) = ty {
                    self.resolve_type(ty);
                }
                self.declare(name, *mutable);
            }
            Stmt::Assign { target, value, .. } => {
                self.resolve_expr(value);
                self.resolve_assign_target(target);
            }
            Stmt::Expr(e) => self.resolve_expr(e),
            Stmt::Return { value, .. } => {
                if let Some(e) = value {
                    self.resolve_expr(e);
                }
            }
            Stmt::If(if_stmt) => self.resolve_if(if_stmt),
            Stmt::While { cond, body, .. } => {
                self.resolve_expr(cond);
                self.resolve_block(body);
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                self.resolve_expr(iter);
                // The loop variable lives in a scope of its own, so it does
                // not leak past the loop.
                self.scopes.push(HashMap::new());
                self.declare(var, false);
                self.resolve_block(body);
                self.scopes.pop();
            }
            Stmt::Block(b) => self.resolve_block(b),
        }
    }

    fn resolve_if(&mut self, if_stmt: &IfStmt) {
        self.resolve_expr(&if_stmt.cond);
        self.resolve_block(&if_stmt.then_block);
        match &if_stmt.else_branch {
            Some(ElseBranch::If(next)) => self.resolve_if(next),
            Some(ElseBranch::Block(b)) => self.resolve_block(b),
            None => {}
        }
    }

    /// Resolve an assignment target and check that it can be assigned to.
    /// The target's names are resolved exactly once, so an unknown name
    /// here reports E0201 and not also E0204.
    fn resolve_assign_target(&mut self, target: &Expr) {
        self.resolve_expr(target);
        let Some(root) = root_var(target) else {
            self.diags.push(
                Diagnostic::error("E0213", "invalid assignment target", target.span)
                    .with_label("cannot assign to this expression")
                    .with_help(
                        "only variables and their fields can be assigned, \
                         like `x = ...` or `user.age = ...`",
                    ),
            );
            return;
        };
        if let Resolution::Local(local) = self.out.resolution(root.id) {
            let def = &self.out.locals[local.0 as usize];
            if !def.mutable {
                let name = def.name.clone();
                self.diags.push(
                    Diagnostic::error(
                        "E0204",
                        format!("cannot assign to `{}`: it is immutable", name),
                        target.span,
                    )
                    .with_label("this variable was not declared as mutable")
                    .with_help(format!(
                        "variables are immutable by default; declare it as `let mut {} = ...`",
                        name
                    )),
                );
            }
        }
    }

    fn resolve_expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Char(_)
            | ExprKind::Str(_)
            | ExprKind::Error => {}
            ExprKind::Var(name) => {
                let resolution = match self.lookup(name) {
                    Some(local) => Resolution::Local(local),
                    None => {
                        let mut d = Diagnostic::error(
                            "E0201",
                            format!("cannot find variable `{}`", name),
                            e.span,
                        )
                        .with_label("not found in this scope");
                        if let Some(similar) = suggest::closest(name, self.visible_locals()) {
                            d = d.with_help(format!("did you mean `{}`?", similar));
                        } else if self.out.func_by_name.contains_key(name) {
                            d = d.with_help(format!(
                                "`{}` is a function; did you mean to call it: `{}(...)`?",
                                name, name
                            ));
                        } else if self.out.enum_by_name.contains_key(name) {
                            d = d.with_help(format!(
                                "`{}` is an enum; name one of its variants: `{}::...`",
                                name, name
                            ));
                        }
                        self.diags.push(d);
                        Resolution::Error
                    }
                };
                self.out.refs.insert(e.id, resolution);
            }
            ExprKind::Unary { operand, .. } => self.resolve_expr(operand),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            ExprKind::Range { lo, hi } => {
                self.resolve_expr(lo);
                self.resolve_expr(hi);
            }
            ExprKind::Call { callee, args } => {
                self.resolve_callee(callee);
                for a in args {
                    self.resolve_expr(a);
                }
            }
            ExprKind::Field { base, .. } => self.resolve_expr(base),
            ExprKind::StructLit { name, fields } => {
                self.resolve_struct_name(name);
                for (_, value) in fields {
                    self.resolve_expr(value);
                }
            }
            ExprKind::Path { base, name } => {
                let resolution = self.resolve_path(base, name);
                self.out.refs.insert(e.id, resolution);
            }
        }
    }

    fn resolve_callee(&mut self, callee: &Expr) {
        let ExprKind::Var(name) = &callee.kind else {
            if !matches!(callee.kind, ExprKind::Error) {
                self.diags.push(
                    Diagnostic::error("E0230", "only named functions can be called", callee.span)
                        .with_label("not a function name")
                        .with_note("methods and function values are not part of the language yet"),
                );
            }
            // Still resolve the sub-expression so its names get checked.
            self.resolve_expr(callee);
            return;
        };

        if let Some(builtin) = Builtin::from_name(name) {
            self.out
                .refs
                .insert(callee.id, Resolution::Builtin(builtin));
            return;
        }
        let resolution = match self.out.func_by_name.get(name) {
            Some(&id) => Resolution::Function(id),
            None => {
                let mut d = Diagnostic::error(
                    "E0202",
                    format!("cannot find function `{}`", name),
                    callee.span,
                )
                .with_label("not defined anywhere in this file");
                let known: Vec<&str> = self
                    .out
                    .func_by_name
                    .keys()
                    .map(String::as_str)
                    .chain(Builtin::ALL.iter().map(|b| b.name()))
                    .collect();
                if let Some(similar) = suggest::closest(name, known) {
                    d = d.with_help(format!("did you mean `{}`?", similar));
                } else if self.lookup(name).is_some() {
                    d = d.with_help(format!("`{}` is a variable, not a function", name));
                }
                self.diags.push(d);
                Resolution::Error
            }
        };
        self.out.refs.insert(callee.id, resolution);
    }

    fn resolve_struct_name(&mut self, name: &Ident) {
        let resolution = if let Some(&id) = self.out.struct_by_name.get(&name.name) {
            Resolution::Struct(id)
        } else if self.out.enum_by_name.contains_key(&name.name) {
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
            Resolution::Error
        } else {
            self.diags.push(
                Diagnostic::error(
                    "E0200",
                    format!("cannot find type `{}`", name.name),
                    name.span,
                )
                .with_label("not a known struct"),
            );
            Resolution::Error
        };
        self.out.refs.insert(name.id, resolution);
    }

    fn resolve_path(&mut self, base: &Ident, name: &Ident) -> Resolution {
        if let Some(&id) = self.out.enum_by_name.get(&base.name) {
            if self.out.enums[id.0].variants.contains(&name.name) {
                return Resolution::Variant(id);
            }
            let variants = self.out.enums[id.0].variants.join(", ");
            self.diags.push(
                Diagnostic::error(
                    "E0216",
                    format!(
                        "no variant `{}` on enum `{}`",
                        name.name, self.out.enums[id.0].name
                    ),
                    name.span,
                )
                .with_label("unknown variant")
                .with_help(format!("available variants: {}", variants)),
            );
            return Resolution::Error;
        }
        if self.out.struct_by_name.contains_key(&base.name) {
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
            return Resolution::Error;
        }
        self.diags.push(
            Diagnostic::error(
                "E0200",
                format!("cannot find type `{}`", base.name),
                base.span,
            )
            .with_label("not a known enum"),
        );
        Resolution::Error
    }
}

/// The variable expression at the root of a field chain, if the expression
/// is a place at all.
fn root_var(e: &Expr) -> Option<&Expr> {
    match &e.kind {
        ExprKind::Var(_) => Some(e),
        ExprKind::Field { base, .. } => root_var(base),
        _ => None,
    }
}
