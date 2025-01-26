// Test AST and AST rewriting
use minijinja::{
    machinery::{ast::*, parse, Span, WhitespaceConfig},
    syntax::SyntaxConfig,
    Value,
};
use std::ops::DerefMut;

fn make_span() -> Span {
    Span {
        start_line: 0,
        start_col: 0,
        start_offset: 0,
        end_line: 0,
        end_col: 0,
        end_offset: 0,
    }
}

/// Rewrite the if expression to always have a false body containing "baz"
fn rewrite_if_expr(expr: &mut IfExpr) {
    let const_el = Const {
        value: Value::from_safe_string("baz".to_string()),
    };

    let false_body = Expr::Const(Spanned::new(const_el, make_span()));

    expr.false_expr = Some(false_body);
}

#[test]
fn test_ast() {
    let template = "{{ 'bar' if foobar }}";

    let mut ast = parse(
        template,
        "fn.tmpl",
        SyntaxConfig::default(),
        WhitespaceConfig::default(),
    )
    .unwrap();

    if let Stmt::Template(ref mut t) = ast {
        for c in t.children.deref_mut() {
            if let Stmt::EmitExpr(e) = c {
                if let Expr::IfExpr(ref mut i) = e.expr {
                    rewrite_if_expr(i.deref_mut());
                }
            }
        }
    }

    insta::assert_debug_snapshot!(ast);
}
