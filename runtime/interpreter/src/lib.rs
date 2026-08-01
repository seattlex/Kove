//! The Kove reference interpreter: a tree-walking evaluator over the AST.
//!
//! Runs only programs that passed the type checker, so evaluation can
//! assume well-typed operands; hitting an impossible combination is a
//! compiler bug and panics with an ICE message. What *is* checked at
//! runtime, and reported as an `E03xx` diagnostic with a source span,
//! is arithmetic safety: division/remainder by zero, Int overflow, and
//! the recursion limit.
//!
//! Values have value semantics: assignment and argument passing copy.
//! This is the deliberate v0.1 memory model; the ownership design that
//! will replace it is tracked in `docs/ownership.md`.

use kove_ast::*;
use kove_diagnostics::{Diagnostic, Span};
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

/// Maximum depth of nested Kove function calls.
pub const RECURSION_LIMIT: usize = 1000;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(Rc<str>),
    Struct {
        name: Rc<str>,
        fields: Vec<(Rc<str>, Value)>,
    },
    EnumVariant {
        enum_name: Rc<str>,
        variant: Rc<str>,
    },
    Range {
        lo: i64,
        hi: i64,
    },
    Unit,
}

impl Value {
    /// Display form used by `println`.
    pub fn display(&self) -> String {
        match self {
            Value::Int(v) => v.to_string(),
            Value::Float(v) => v.to_string(),
            Value::Bool(v) => v.to_string(),
            Value::Char(c) => c.to_string(),
            Value::Str(s) => s.to_string(),
            Value::Struct { name, fields } => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(n, v)| format!("{}: {}", n, v.display()))
                    .collect();
                format!("{} {{ {} }}", name, inner.join(", "))
            }
            Value::EnumVariant { enum_name, variant } => format!("{}::{}", enum_name, variant),
            Value::Range { lo, hi } => format!("{}..{}", lo, hi),
            Value::Unit => "()".to_string(),
        }
    }
}

/// A runtime failure, carrying a renderable diagnostic.
///
/// The diagnostic is boxed because every evaluation step returns a
/// `Result` with this as its error type, and a bare `Diagnostic` would
/// make each of those results 128 bytes wide for a case that almost never
/// happens.
#[derive(Debug)]
pub struct RuntimeError {
    diagnostic: Box<Diagnostic>,
}

impl RuntimeError {
    fn new(code: &'static str, message: impl Into<String>, span: Span) -> RuntimeError {
        RuntimeError::from(Diagnostic::error(code, message, span))
    }

    fn with_note(mut self, note: impl Into<String>) -> RuntimeError {
        self.diagnostic = Box::new(self.diagnostic.with_note(note));
        self
    }

    /// The diagnostic describing the failure.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    /// Consume the error and take its diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        *self.diagnostic
    }
}

impl From<Diagnostic> for RuntimeError {
    fn from(diagnostic: Diagnostic) -> RuntimeError {
        RuntimeError {
            diagnostic: Box::new(diagnostic),
        }
    }
}

enum Flow {
    Normal,
    Return(Value),
}

type Exec = Result<Flow, RuntimeError>;
type Eval = Result<Value, RuntimeError>;

/// Run a type-checked program's `main`, writing `println` output to `out`.
pub fn run(program: &Program, out: &mut dyn Write) -> Result<(), RuntimeError> {
    run_function(program, "main", out)
}

/// Run one named function that takes no arguments.
///
/// `kove test` uses this to call each test function in turn. The driver
/// has already verified the function exists and takes no parameters.
pub fn run_function(
    program: &Program,
    name: &str,
    out: &mut dyn Write,
) -> Result<(), RuntimeError> {
    let mut funcs: HashMap<&str, &Function> = HashMap::new();
    for item in &program.items {
        if let Item::Function(f) = item {
            funcs.insert(f.name.name.as_str(), f);
        }
    }
    let entry = funcs
        .get(name)
        .copied()
        .unwrap_or_else(|| ice("asked to run a function that does not exist"));
    let mut interp = Interp {
        funcs,
        out,
        depth: 0,
    };
    interp.call(entry, Vec::new(), entry.name.span)?;
    Ok(())
}

struct Interp<'p, 'o> {
    funcs: HashMap<&'p str, &'p Function>,
    out: &'o mut dyn Write,
    depth: usize,
}

/// Lexical scopes of one function activation.
struct Env {
    scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    fn new() -> Env {
        Env { scopes: Vec::new() }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, value: Value) {
        self.scopes
            .last_mut()
            .expect("env scope stack is never empty")
            .insert(name.to_string(), value);
    }

    fn get(&self, name: &str) -> &Value {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .expect("the type checker verified every variable reference")
    }

    fn get_mut(&mut self, name: &str) -> &mut Value {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|s| s.get_mut(name))
            .expect("the type checker verified every assignment target")
    }
}

impl<'p, 'o> Interp<'p, 'o> {
    fn call(&mut self, f: &'p Function, args: Vec<Value>, call_span: Span) -> Eval {
        if self.depth >= RECURSION_LIMIT {
            return Err(RuntimeError::new(
                "E0304",
                format!(
                    "recursion limit ({}) reached while calling `{}`",
                    RECURSION_LIMIT, f.name.name
                ),
                call_span,
            ));
        }
        self.depth += 1;
        let mut env = Env::new();
        env.push();
        for (param, value) in f.params.iter().zip(args) {
            env.declare(&param.name.name, value);
        }
        let flow = self.exec_block(&f.body, &mut env)?;
        self.depth -= 1;
        Ok(match flow {
            Flow::Return(v) => v,
            Flow::Normal => Value::Unit,
        })
    }

    fn exec_block(&mut self, block: &'p Block, env: &mut Env) -> Exec {
        env.push();
        for stmt in &block.stmts {
            match self.exec_stmt(stmt, env) {
                Ok(Flow::Normal) => {}
                other => {
                    env.pop();
                    return other;
                }
            }
        }
        env.pop();
        Ok(Flow::Normal)
    }

    fn exec_stmt(&mut self, stmt: &'p Stmt, env: &mut Env) -> Exec {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let v = self.eval(value, env)?;
                env.declare(&name.name, v);
                Ok(Flow::Normal)
            }
            Stmt::Assign { target, value, .. } => {
                let v = self.eval(value, env)?;
                self.assign(target, v, env)?;
                Ok(Flow::Normal)
            }
            Stmt::Expr(e) => {
                self.eval(e, env)?;
                Ok(Flow::Normal)
            }
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Unit,
                };
                Ok(Flow::Return(v))
            }
            Stmt::If(if_stmt) => self.exec_if(if_stmt, env),
            Stmt::While { cond, body, .. } => {
                while self.eval_bool(cond, env)? {
                    match self.exec_block(body, env)? {
                        Flow::Normal => {}
                        flow => return Ok(flow),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                let Value::Range { lo, hi } = self.eval(iter, env)? else {
                    ice("for-loop iterable was not a range")
                };
                for i in lo..hi {
                    env.push();
                    env.declare(&var.name, Value::Int(i));
                    let flow = self.exec_block(body, env);
                    env.pop();
                    match flow? {
                        Flow::Normal => {}
                        flow => return Ok(flow),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Block(b) => self.exec_block(b, env),
        }
    }

    fn exec_if(&mut self, if_stmt: &'p IfStmt, env: &mut Env) -> Exec {
        if self.eval_bool(&if_stmt.cond, env)? {
            self.exec_block(&if_stmt.then_block, env)
        } else {
            match &if_stmt.else_branch {
                Some(ElseBranch::If(next)) => self.exec_if(next, env),
                Some(ElseBranch::Block(b)) => self.exec_block(b, env),
                None => Ok(Flow::Normal),
            }
        }
    }

    fn eval_bool(&mut self, e: &'p Expr, env: &mut Env) -> Result<bool, RuntimeError> {
        match self.eval(e, env)? {
            Value::Bool(b) => Ok(b),
            _ => ice("condition did not evaluate to Bool"),
        }
    }

    fn eval(&mut self, e: &'p Expr, env: &mut Env) -> Eval {
        match &e.kind {
            ExprKind::Int(v) => Ok(Value::Int(*v)),
            ExprKind::Float(v) => Ok(Value::Float(*v)),
            ExprKind::Bool(v) => Ok(Value::Bool(*v)),
            ExprKind::Char(c) => Ok(Value::Char(*c)),
            ExprKind::Str(s) => Ok(Value::Str(Rc::from(s.as_str()))),
            ExprKind::Var(name) => Ok(env.get(name).clone()),
            ExprKind::Unary { op, operand } => {
                let v = self.eval(operand, env)?;
                match (op, v) {
                    (UnaryOp::Neg, Value::Int(i)) => i
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| overflow("negate", e.span)),
                    (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
                    (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    _ => ice("unary operand had the wrong type"),
                }
            }
            ExprKind::Binary {
                op,
                lhs,
                rhs,
                op_span,
            } => self.eval_binary(*op, lhs, rhs, *op_span, env),
            ExprKind::Range { lo, hi } => {
                let (Value::Int(lo), Value::Int(hi)) = (self.eval(lo, env)?, self.eval(hi, env)?)
                else {
                    ice("range bounds were not Ints")
                };
                Ok(Value::Range { lo, hi })
            }
            ExprKind::Call { callee, args } => {
                let ExprKind::Var(name) = &callee.kind else {
                    ice("call to a non-identifier callee")
                };
                let mut values = Vec::with_capacity(args.len());
                for a in args {
                    values.push(self.eval(a, env)?);
                }
                if name == "println" {
                    let value = values.into_iter().next().unwrap_or(Value::Unit);
                    writeln!(self.out, "{}", value.display()).map_err(|err| {
                        RuntimeError::new(
                            "E0305",
                            format!("failed to write output: {}", err),
                            e.span,
                        )
                    })?;
                    return Ok(Value::Unit);
                }
                if name == "to_float" {
                    let Some(Value::Int(v)) = values.into_iter().next() else {
                        ice("to_float was given something other than an Int")
                    };
                    // Beyond 2^53 this rounds; that is what the hardware
                    // does and what every language with both types does.
                    return Ok(Value::Float(v as f64));
                }
                if name == "to_int" {
                    let Some(Value::Float(v)) = values.into_iter().next() else {
                        ice("to_int was given something other than a Float")
                    };
                    let span = args.first().map(|a| a.span).unwrap_or(e.span);
                    if v.is_nan() {
                        return Err(RuntimeError::new(
                            "E0307",
                            "cannot convert this Float to Int: it is not a number",
                            span,
                        ));
                    }
                    // Truncation is toward zero, so the check is against
                    // the range a truncated value could land in.
                    let truncated = v.trunc();
                    if truncated < i64::MIN as f64 || truncated > i64::MAX as f64 {
                        return Err(RuntimeError::new(
                            "E0307",
                            format!("cannot convert `{}` to Int: it is out of range", v),
                            span,
                        )
                        .with_note(format!(
                            "`Int` values range from {} to {}",
                            i64::MIN,
                            i64::MAX
                        )));
                    }
                    return Ok(Value::Int(truncated as i64));
                }
                if name == "assert" {
                    let Some(Value::Bool(ok)) = values.into_iter().next() else {
                        ice("assert was given something other than a Bool")
                    };
                    if !ok {
                        // The span is the condition itself, so the caret
                        // points at what failed rather than at the call.
                        let span = args.first().map(|a| a.span).unwrap_or(e.span);
                        return Err(RuntimeError::new("E0306", "assertion failed", span)
                            .with_note("this condition evaluated to `false`"));
                    }
                    return Ok(Value::Unit);
                }
                let f = *self
                    .funcs
                    .get(name.as_str())
                    .unwrap_or_else(|| ice("call to an unknown function"));
                self.call(f, values, e.span)
            }
            ExprKind::Field { base, name } => {
                let Value::Struct { fields, .. } = self.eval(base, env)? else {
                    ice("field access on a non-struct value")
                };
                fields
                    .iter()
                    .find(|(n, _)| n.as_ref() == name.name)
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| ice("struct value was missing a checked field"))
            }
            ExprKind::StructLit { name, fields } => {
                let mut values = Vec::with_capacity(fields.len());
                for (fname, fexpr) in fields {
                    let v = self.eval(fexpr, env)?;
                    values.push((Rc::from(fname.name.as_str()), v));
                }
                Ok(Value::Struct {
                    name: Rc::from(name.name.as_str()),
                    fields: values,
                })
            }
            ExprKind::Path { base, name } => Ok(Value::EnumVariant {
                enum_name: Rc::from(base.name.as_str()),
                variant: Rc::from(name.name.as_str()),
            }),
            ExprKind::Error => ice("an error placeholder reached the interpreter"),
        }
    }

    fn eval_binary(
        &mut self,
        op: BinaryOp,
        lhs: &'p Expr,
        rhs: &'p Expr,
        op_span: Span,
        env: &mut Env,
    ) -> Eval {
        use BinaryOp::*;
        // Short-circuit forms first.
        if op == And {
            return Ok(Value::Bool(
                self.eval_bool(lhs, env)? && self.eval_bool(rhs, env)?,
            ));
        }
        if op == Or {
            return Ok(Value::Bool(
                self.eval_bool(lhs, env)? || self.eval_bool(rhs, env)?,
            ));
        }
        let l = self.eval(lhs, env)?;
        let r = self.eval(rhs, env)?;
        match op {
            Eq => return Ok(Value::Bool(l == r)),
            Ne => return Ok(Value::Bool(l != r)),
            _ => {}
        }
        match (l, r) {
            (Value::Int(a), Value::Int(b)) => match op {
                Add => a
                    .checked_add(b)
                    .map(Value::Int)
                    .ok_or_else(|| overflow("add", op_span)),
                Sub => a
                    .checked_sub(b)
                    .map(Value::Int)
                    .ok_or_else(|| overflow("subtract", op_span)),
                Mul => a
                    .checked_mul(b)
                    .map(Value::Int)
                    .ok_or_else(|| overflow("multiply", op_span)),
                Div => {
                    if b == 0 {
                        Err(RuntimeError::new(
                            "E0301",
                            "attempted to divide by zero",
                            op_span,
                        ))
                    } else {
                        a.checked_div(b)
                            .map(Value::Int)
                            .ok_or_else(|| overflow("divide", op_span))
                    }
                }
                Rem => {
                    if b == 0 {
                        Err(RuntimeError::new(
                            "E0303",
                            "attempted to take a remainder by zero",
                            op_span,
                        ))
                    } else {
                        a.checked_rem(b)
                            .map(Value::Int)
                            .ok_or_else(|| overflow("take the remainder of", op_span))
                    }
                }
                Lt => Ok(Value::Bool(a < b)),
                Le => Ok(Value::Bool(a <= b)),
                Gt => Ok(Value::Bool(a > b)),
                Ge => Ok(Value::Bool(a >= b)),
                _ => ice("unhandled Int binary operator"),
            },
            (Value::Float(a), Value::Float(b)) => match op {
                Add => Ok(Value::Float(a + b)),
                Sub => Ok(Value::Float(a - b)),
                Mul => Ok(Value::Float(a * b)),
                Div => Ok(Value::Float(a / b)),
                Rem => Ok(Value::Float(a % b)),
                Lt => Ok(Value::Bool(a < b)),
                Le => Ok(Value::Bool(a <= b)),
                Gt => Ok(Value::Bool(a > b)),
                Ge => Ok(Value::Bool(a >= b)),
                _ => ice("unhandled Float binary operator"),
            },
            // Rust's Ord for char is Unicode scalar order, which is the
            // ordering the type checker promised.
            (Value::Char(a), Value::Char(b)) => match op {
                Lt => Ok(Value::Bool(a < b)),
                Le => Ok(Value::Bool(a <= b)),
                Gt => Ok(Value::Bool(a > b)),
                Ge => Ok(Value::Bool(a >= b)),
                _ => ice("unhandled Char binary operator"),
            },
            _ => ice("binary operands had mismatched types"),
        }
    }

    /// Assign through a variable or a `var.field.field` chain. The type
    /// checker already verified the target exists and the root is mutable.
    fn assign(
        &mut self,
        target: &'p Expr,
        value: Value,
        env: &mut Env,
    ) -> Result<(), RuntimeError> {
        // Collect the field path from outermost access down to the root var.
        let mut path: Vec<&str> = Vec::new();
        let mut cur = target;
        loop {
            match &cur.kind {
                ExprKind::Var(name) => {
                    let mut slot = env.get_mut(name);
                    for field in path.iter().rev() {
                        let Value::Struct { fields, .. } = slot else {
                            ice("field assignment through a non-struct value")
                        };
                        slot = fields
                            .iter_mut()
                            .find(|(n, _)| n.as_ref() == *field)
                            .map(|(_, v)| v)
                            .unwrap_or_else(|| ice("struct value was missing a checked field"));
                    }
                    *slot = value;
                    return Ok(());
                }
                ExprKind::Field { base, name } => {
                    path.push(&name.name);
                    cur = base;
                }
                _ => ice("assignment to a non-place expression"),
            }
        }
    }
}

fn overflow(action: &str, span: Span) -> RuntimeError {
    Diagnostic::error(
        "E0302",
        format!("attempt to {} with overflow", action),
        span,
    )
    .with_note(format!(
        "`Int` values range from {} to {}",
        i64::MIN,
        i64::MAX
    ))
    .into()
}

/// Internal compiler error: the type checker let something through that it
/// should not have. Panicking (rather than a user diagnostic) is deliberate.
fn ice(msg: &str) -> ! {
    panic!(
        "internal compiler error (interpreter): {msg}; this is a bug in Kove, not in your program"
    );
}
