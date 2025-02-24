#![cfg(feature = "unstable_machinery")]
use std::collections::BTreeMap;

use minijinja::machinery::ast::Var;
use minijinja::machinery::{CodeGenerator, Instruction};
use minijinja::value::Value;

#[test]
fn test_for_loop() {
    let mut c = CodeGenerator::new("<unknown>", "");
    let string_id = c.register_str("items");
    c.add(Instruction::Lookup(string_id));
    c.start_for_loop(true, false);
    c.add(Instruction::Emit);
    c.end_for_loop(false);
    let string_id = c.register_str("!");
    c.add(Instruction::EmitRaw(string_id));

    insta::assert_debug_snapshot!(&c.finish());
}

#[test]
fn test_if_branches() {
    let mut c = CodeGenerator::new("<unknown>", "");
    let string_id = c.register_str("false");
    c.add(Instruction::Lookup(string_id));
    c.start_if();
    let string_id = c.register_str("nope1");
    c.add(Instruction::EmitRaw(string_id));
    c.start_else();
    let string_id = c.register_str("nil");
    c.add(Instruction::Lookup(string_id));
    c.start_if();
    let string_id = c.register_str("nope1");
    c.add(Instruction::EmitRaw(string_id));
    c.start_else();
    let string_id = c.register_str("yes");
    c.add(Instruction::EmitRaw(string_id));
    c.end_if();
    c.end_if();

    insta::assert_debug_snapshot!(&c.finish());
}

#[test]
fn test_bool_ops() {
    let mut c = CodeGenerator::new("<unknown>", "");

    c.start_sc_bool();
    let string_id = c.register_str("first");
    c.add(Instruction::Lookup(string_id));
    c.sc_bool(true);
    let string_id = c.register_str("second");
    c.add(Instruction::Lookup(string_id));
    c.sc_bool(false);
    let string_id = c.register_str("third");
    c.add(Instruction::Lookup(string_id));
    c.end_sc_bool();

    insta::assert_debug_snapshot!(&c.finish());
}

#[test]
fn test_const() {
    let mut c = CodeGenerator::new("<unknown>", "");

    c.add_const(Value::from("a"));
    c.add_const(Value::from(42));
    c.add(Instruction::StringConcat);

    insta::assert_debug_snapshot!(&c.finish());
}

#[test]
fn test_referenced_names_empty_bug() {
    let c = CodeGenerator::new("<unknown>", "");
    let instructions = c.finish().0;
    let rv = instructions.get_referenced_names(0);
    assert!(rv.is_empty());
}

#[test]
fn test_const_folding() {
    use minijinja::machinery::{
        ast::{BinOp, BinOpKind, Const, Expr, List, Map, Spanned, UnaryOp, UnaryOpKind},
        Span,
    };
    use minijinja::Value;

    // Simple constant
    let const_expr = Expr::Const(Spanned::new(
        Const {
            value: Value::from(42),
        },
        Span::default(),
    ));
    assert_eq!(const_expr.as_const(), Some(Value::from(42)));

    // List of constants
    let list_expr = Expr::List(Spanned::new(
        List {
            items: vec![
                Expr::Const(Spanned::new(
                    Const {
                        value: Value::from(1),
                    },
                    Span::default(),
                )),
                Expr::Const(Spanned::new(
                    Const {
                        value: Value::from(2),
                    },
                    Span::default(),
                )),
            ],
        },
        Span::default(),
    ));
    assert_eq!(
        list_expr.as_const(),
        Some(Value::from(vec![Value::from(1), Value::from(2)]))
    );

    // Map of constants
    let map_expr = Expr::Map(Spanned::new(
        Map {
            keys: vec![Expr::Const(Spanned::new(
                Const {
                    value: Value::from("a"),
                },
                Span::default(),
            ))],
            values: vec![Expr::Const(Spanned::new(
                Const {
                    value: Value::from(1),
                },
                Span::default(),
            ))],
        },
        Span::default(),
    ));
    let mut expected_map = BTreeMap::new();
    expected_map.insert(Value::from("a"), Value::from(1));
    assert_eq!(map_expr.as_const(), Some(Value::from(expected_map)));

    // Binary op with constants
    let binop_expr = Expr::BinOp(Spanned::new(
        BinOp {
            op: BinOpKind::Add,
            left: Expr::Const(Spanned::new(
                Const {
                    value: Value::from(1),
                },
                Span::default(),
            )),
            right: Expr::Const(Spanned::new(
                Const {
                    value: Value::from(2),
                },
                Span::default(),
            )),
        },
        Span::default(),
    ));
    assert_eq!(binop_expr.as_const(), Some(Value::from(3)));

    // Unary op with constant
    let unaryop_expr = Expr::UnaryOp(Spanned::new(
        UnaryOp {
            op: UnaryOpKind::Not,
            expr: Expr::Const(Spanned::new(
                Const {
                    value: Value::from(false),
                },
                Span::default(),
            )),
        },
        Span::default(),
    ));
    assert_eq!(unaryop_expr.as_const(), Some(Value::from(true)));

    // Test cases that should return None

    // List with var
    let list_expr = Expr::List(Spanned::new(
        List {
            items: vec![
                Expr::Const(Spanned::new(
                    Const {
                        value: Value::from(1),
                    },
                    Span::default(),
                )),
                Expr::Var(Spanned::new(Var { id: "foo" }, Span::default())),
            ],
        },
        Span::default(),
    ));
    assert_eq!(list_expr.as_const(), None);

    // Binary op with non-constant
    let binop_expr = Expr::BinOp(Spanned::new(
        BinOp {
            op: BinOpKind::Add,
            left: Expr::Const(Spanned::new(
                Const {
                    value: Value::from(1),
                },
                Span::default(),
            )),
            right: Expr::Var(Spanned::new(Var { id: "foo" }, Span::default())),
        },
        Span::default(),
    ));
    assert_eq!(binop_expr.as_const(), None);
}
