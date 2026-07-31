//! Parse tree tests: golden s-expressions for representative programs,
//! error recovery shapes, the struct-literal/condition interplay, and
//! incremental reparse consistency.

use kove_tests::{parse, sexpr};
use reparse::TextEdit;

#[track_caller]
fn assert_tree(src: &str, expected: &str) {
    assert_eq!(sexpr(src).trim(), expected.trim(), "source: {src:?}");
}

#[test]
fn function_with_params_and_return_type() {
    assert_tree(
        "fn add(a: Int, b: Int) -> Int {\n    return a + b * 2;\n}\n",
        r#"
(ROOT
  (source_file
    (function_declaration 'fn' name: 'add'
      params: (parameter_list '('
        (parameter name: 'a' ':'
          type: (type_expression name: 'Int')) ','
        (parameter name: 'b' ':'
          type: (type_expression name: 'Int')) ')') '->'
      return_type: (type_expression name: 'Int')
      body: (block '{'
        (return_statement 'return'
          value: (binary_expression lhs: 'a' op: '+'
            rhs: (binary_expression lhs: 'b' op: '*' rhs: '2')) ';') '}'))))
"#,
    );
}

#[test]
fn empty_block_after_bare_identifier_condition() {
    // The classic ambiguity: `if ready { }` must be a condition + block,
    // never a struct literal `ready { }`.
    assert_tree(
        "fn main() { let ready = true; if ready { } }",
        r#"
(ROOT
  (source_file
    (function_declaration 'fn' name: 'main'
      params: (parameter_list '(' ')')
      body: (block '{'
        (let_statement 'let' name: 'ready' '=' value: 'true' ';')
        (if_statement 'if' condition: 'ready'
          then: (block '{' '}')) '}'))))
"#,
    );
}

#[test]
fn struct_literal_in_expression_position() {
    assert_tree(
        "struct User { name: String }\nfn main() { let u = User { name: \"A\" }; println(u.name); }",
        r#"
(ROOT
  (source_file
    (struct_declaration 'struct' name: 'User' '{'
      (field_declaration name: 'name' ':'
        type: (type_expression name: 'String')) '}')
    (function_declaration 'fn' name: 'main'
      params: (parameter_list '(' ')')
      body: (block '{'
        (let_statement 'let' name: 'u' '='
          value: (struct_literal name: 'User' '{'
            (field_initializer name: 'name' ':' value: '"A"') '}') ';')
        (expression_statement
          value: (call_expression callee: 'println'
            (arguments '('
              arg: (field_expression base: 'u' '.' name: 'name') ')')) ';') '}'))))
"#,
    );
}

#[test]
fn enum_declaration_and_path_expression() {
    assert_tree(
        "enum Status { Active }\nfn main() { let s = Status::Active; }",
        r#"
(ROOT
  (source_file
    (enum_declaration 'enum' name: 'Status' '{'
      (variant name: 'Active') '}')
    (function_declaration 'fn' name: 'main'
      params: (parameter_list '(' ')')
      body: (block '{'
        (let_statement 'let' name: 's' '='
          value: (path_expression base: 'Status' '::' name: 'Active') ';') '}'))))
"#,
    );
}

#[test]
fn for_loop_over_range() {
    assert_tree(
        "fn main() { for i in 0..3 { println(i); } }",
        r#"
(ROOT
  (source_file
    (function_declaration 'fn' name: 'main'
      params: (parameter_list '(' ')')
      body: (block '{'
        (for_statement 'for' name: 'i' 'in'
          iter: (range_expression lhs: '0' op: '..' rhs: '3')
          body: (block '{'
            (expression_statement
              value: (call_expression callee: 'println'
                (arguments '(' arg: 'i' ')')) ';') '}')) '}'))))
"#,
    );
}

#[test]
fn recovery_still_produces_a_complete_tree() {
    // Broken input parses to a full tree with zero-width missing tokens.
    assert_tree(
        "fn main() { foo(bar",
        r#"
(ROOT
  (source_file
    (function_declaration 'fn' name: 'main'
      params: (parameter_list '(' ')')
      body: (block '{'
        (expression_statement
          value: (call_expression callee: 'foo'
            (arguments '(' arg: 'bar' (missing ')'))) (missing ';')) (missing '}')))))
"#,
    );
}

#[test]
fn every_input_produces_a_tree_covering_every_byte() {
    for src in [
        "",
        "fn",
        "fn main( {",
        "struct { let while",
        "if if if",
        "fn main() { let x = ((((1; }",
        "@#$%^&",
    ] {
        let doc = parse(src);
        assert_eq!(
            doc.tree().width() as usize,
            src.len(),
            "tree must cover every byte of {src:?}"
        );
    }
}

#[test]
fn precedence_and_associativity() {
    // `a - b - c` associates left; `||` binds looser than `&&`.
    // Compare with whitespace collapsed; nesting, not layout, is under test.
    let tree = sexpr("fn f() { let x = a - b - c; let y = p || q && r; }");
    let flat = tree.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains(
            "value: (binary_expression lhs: (binary_expression lhs: 'a' op: '-' rhs: 'b') op: '-' rhs: 'c')"
        ),
        "left associativity:\n{tree}"
    );
    assert!(
        flat.contains(
            "value: (binary_expression lhs: 'p' op: '||' rhs: (binary_expression lhs: 'q' op: '&&' rhs: 'r'))"
        ),
        "|| looser than &&:\n{tree}"
    );
}

#[test]
fn else_if_chains_nest() {
    let tree = sexpr("fn f(x: Int) { if x > 1 { } else if x > 0 { } else { } }");
    assert!(tree.contains("else: (if_statement"), "{tree}");
    assert!(tree.contains("else: (block"), "{tree}");
}

#[test]
fn a_trailing_comma_is_allowed_in_every_list() {
    // Consistency: anywhere a comma separates items, a trailing one is
    // accepted, so a wrapped list can end every line the same way.
    for src in [
        "fn f() { g(1, 2,); }",
        "fn f(a: Int, b: Int,) { }",
        "struct S { a: Int, b: Int, }",
        "enum E { A, B, }",
        "struct S { a: Int }\nfn f() { let s = S { a: 1, }; }",
    ] {
        let doc = parse(src);
        assert!(
            !doc.tree().has_errors(),
            "should parse cleanly: {src:?}\n{}",
            sexpr(src)
        );
    }
}

#[test]
fn assignment_targets() {
    let tree = sexpr("fn f() { x = 1; user.age = 2; }");
    assert!(tree.contains("(assignment_statement target: 'x'"), "{tree}");
    assert!(
        tree.contains("target: (field_expression base: 'user' '.' name: 'age')"),
        "{tree}"
    );
}

#[test]
fn incremental_edit_matches_from_scratch_parse() {
    // Type the missing `)` into `foo(bar`. The incrementally repaired
    // tree must equal a fresh parse of the new text.
    let mut doc = parse("fn main() { foo(bar; }");
    let insert_at = doc.text().find("bar").unwrap() + 3;
    doc.edit(TextEdit::insert(insert_at as u32, ")"));
    let fresh = parse(doc.text());
    assert_eq!(
        doc.tree().root().to_sexpr(doc.language(), doc.text()),
        fresh.tree().root().to_sexpr(fresh.language(), fresh.text())
    );
    assert!(!doc.tree().has_errors());
}
