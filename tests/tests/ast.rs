//! AST tests: lowering produces the right shapes, spans point at real
//! source, and literal decoding reports its own diagnostics.

use kove_ast::*;
use kove_tests::codes;

fn program(src: &str) -> Program {
    let c = kove_cli::compile("test.kov", src);
    c.program
}

#[test]
fn items_lower_to_the_right_variants() {
    let p = program(
        "struct User { name: String }\n\
         enum Status { Active, Banned }\n\
         fn main() { }\n",
    );
    assert_eq!(p.items.len(), 3);
    assert!(matches!(&p.items[0], Item::Struct(s) if s.name.name == "User"));
    assert!(matches!(&p.items[1], Item::Enum(e) if e.variants.len() == 2));
    assert!(matches!(&p.items[2], Item::Function(f) if f.name.name == "main"));
}

#[test]
fn function_signature_lowering() {
    let p = program("fn add(a: Int, b: Int) -> Int { return a + b; }");
    let Item::Function(f) = &p.items[0] else {
        panic!("expected a function");
    };
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name.name, "a");
    assert_eq!(f.params[1].ty.name, "Int");
    assert_eq!(f.return_type.as_ref().unwrap().name, "Int");
    assert_eq!(f.body.stmts.len(), 1);
    assert!(matches!(
        &f.body.stmts[0],
        Stmt::Return { value: Some(_), .. }
    ));
}

#[test]
fn spans_point_back_at_source() {
    let src = "fn main() { let answer = 42; }";
    let p = program(src);
    let Item::Function(f) = &p.items[0] else {
        panic!("expected a function");
    };
    let Stmt::Let { name, value, .. } = &f.body.stmts[0] else {
        panic!("expected a let");
    };
    assert_eq!(
        &src[name.span.start as usize..name.span.end as usize],
        "answer"
    );
    assert_eq!(
        &src[value.span.start as usize..value.span.end as usize],
        "42"
    );
}

#[test]
fn mutability_and_annotations_lower() {
    let p = program("fn f() { let mut n: Int = 0; let s = \"x\"; }");
    let Item::Function(f) = &p.items[0] else {
        panic!("expected a function");
    };
    let Stmt::Let { mutable, ty, .. } = &f.body.stmts[0] else {
        panic!("expected a let");
    };
    assert!(*mutable);
    assert_eq!(ty.as_ref().unwrap().name, "Int");
    let Stmt::Let { mutable, ty, .. } = &f.body.stmts[1] else {
        panic!("expected a let");
    };
    assert!(!*mutable);
    assert!(ty.is_none());
}

#[test]
fn string_escapes_decode() {
    let p = program(r#"fn f() { let s = "a\tb\n\"q\""; let c = '\n'; }"#);
    let Item::Function(f) = &p.items[0] else {
        panic!("expected a function");
    };
    let Stmt::Let { value, .. } = &f.body.stmts[0] else {
        panic!("expected a let");
    };
    assert!(matches!(&value.kind, ExprKind::Str(s) if s == "a\tb\n\"q\""));
    let Stmt::Let { value, .. } = &f.body.stmts[1] else {
        panic!("expected a let");
    };
    assert!(matches!(&value.kind, ExprKind::Char('\n')));
}

#[test]
fn parenthesized_expressions_disappear() {
    let p = program("fn f() { let x = (1 + 2) * 3; }");
    let Item::Function(f) = &p.items[0] else {
        panic!("expected a function");
    };
    let Stmt::Let { value, .. } = &f.body.stmts[0] else {
        panic!("expected a let");
    };
    // In (1 + 2) * 3 the parens change nesting but produce no AST node.
    let ExprKind::Binary {
        op: BinaryOp::Mul,
        lhs,
        ..
    } = &value.kind
    else {
        panic!("expected a multiplication at the top");
    };
    assert!(matches!(
        &lhs.kind,
        ExprKind::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn compound_assignment_desugars_to_a_plain_assignment() {
    let p = program("fn f() { let mut x = 0; x += 2; }");
    let Item::Function(f) = &p.items[0] else {
        panic!("expected a function");
    };
    let Stmt::Assign { target, value, .. } = &f.body.stmts[1] else {
        panic!("expected an assignment");
    };
    assert!(matches!(&target.kind, ExprKind::Var(n) if n == "x"));
    // The value is `x + 2`, built here rather than anywhere later.
    let ExprKind::Binary { op, lhs, rhs, .. } = &value.kind else {
        panic!("expected a binary expression, got {:?}", value.kind);
    };
    assert_eq!(*op, BinaryOp::Add);
    assert!(matches!(&lhs.kind, ExprKind::Var(n) if n == "x"));
    assert!(matches!(&rhs.kind, ExprKind::Int(2)));
    // The two copies of the target are distinct nodes, which matters
    // because the resolver keys references by id.
    assert_ne!(target.id, lhs.id);
}

#[test]
fn every_compound_operator_desugars() {
    for (src, want) in [
        ("x += 1", BinaryOp::Add),
        ("x -= 1", BinaryOp::Sub),
        ("x *= 1", BinaryOp::Mul),
        ("x /= 1", BinaryOp::Div),
        ("x %= 1", BinaryOp::Rem),
    ] {
        let p = program(&format!("fn f() {{ let mut x = 0; {src}; }}"));
        let Item::Function(f) = &p.items[0] else {
            panic!("expected a function");
        };
        let Stmt::Assign { value, .. } = &f.body.stmts[1] else {
            panic!("expected an assignment");
        };
        let ExprKind::Binary { op, .. } = &value.kind else {
            panic!("expected a binary expression for {src}");
        };
        assert_eq!(*op, want, "for {src}");
    }
}

#[test]
fn integer_overflow_is_reported() {
    assert!(codes("fn main() { let x = 99999999999999999999; }").contains(&"E0110"));
}

#[test]
fn unknown_escape_is_reported() {
    assert!(codes(r#"fn main() { let s = "a\qb"; }"#).contains(&"E0111"));
    // ...with a span on the escape itself.
    let src = r#"fn main() { let s = "a\qb"; }"#;
    let c = kove_cli::compile("test.kov", src);
    let d = c.diagnostics.iter().find(|d| d.code == "E0111").unwrap();
    assert_eq!(&src[d.span.start as usize..d.span.end as usize], r"\q");
}

#[test]
fn top_level_statement_is_reported() {
    assert!(codes("let x = 1;").contains(&"E0104"));
    assert!(codes("println(\"hi\");").contains(&"E0104"));
}
