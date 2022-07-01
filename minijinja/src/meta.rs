//! Provides meta utilities for working with templates.
//!
//! The meta API has some uses for limited introspection that can help optimize
//! how templates are interacted with.  For instance it can be used to detect
//! which templates are reachable from a set of known templates so that the
//! environment can be appropriately initialized.  Likewise it can be used to
//! identify variables that need to be supplied into the context based on what
//! templates are using.
use std::collections::HashSet;

use crate::ast;
use crate::error::Error;
use crate::parser::parse;

/// Given a template source returns a set of undeclared variables.
///
/// Returns a set of all variables in the template that will be looked up from
/// the context at runtime. Because at compile time it’s not known which
/// variables will be used depending on the path the execution takes at runtime,
/// all variables are returned.  Only variables that are known to be declared
/// in the template itself are not returned.
///
/// # Example
///
/// ```rust
/// # use minijinja::meta::find_undeclared_variables;
/// let names = find_undeclared_variables(r#"
///     {% for item in seq %}{{ loop.index}}: {{ item }} ({{ hint }}){% endfor %}
/// "#).unwrap();
/// assert!(names.contains("hint"));
/// assert!(!names.contains("item"));
/// assert!(!names.contains("loop"));
/// assert!(names.contains("seq"));
/// ```
pub fn find_undeclared_variables(source: &str) -> Result<HashSet<String>, Error> {
    struct State {
        out: HashSet<String>,
        assigned: Vec<HashSet<String>>,
    }

    impl State {
        fn is_assigned(&self, name: &str) -> bool {
            self.assigned.iter().any(|x| x.contains(name))
        }

        fn assign(&mut self, name: &str) {
            self.assigned.last_mut().unwrap().insert(name.to_string());
        }

        fn push(&mut self) {
            self.assigned.push(Default::default());
        }

        fn pop(&mut self) {
            self.assigned.pop();
        }
    }

    fn visit_expr(expr: &ast::Expr, state: &mut State) {
        match expr {
            ast::Expr::Var(var) => {
                if !state.is_assigned(var.id) {
                    state.out.insert(var.id.to_string());
                    state.assign(var.id);
                }
            }
            ast::Expr::Const(_) => {}
            ast::Expr::UnaryOp(expr) => visit_expr(&expr.expr, state),
            ast::Expr::BinOp(expr) => {
                visit_expr(&expr.left, state);
                visit_expr(&expr.right, state);
            }
            ast::Expr::IfExpr(expr) => {
                visit_expr(&expr.test_expr, state);
                visit_expr(&expr.true_expr, state);
                if let Some(ref false_expr) = expr.false_expr {
                    visit_expr(false_expr, state);
                }
            }
            ast::Expr::Filter(expr) => {
                if let Some(ref expr) = expr.expr {
                    visit_expr(expr, state);
                }
                for arg in &expr.args {
                    visit_expr(arg, state);
                }
            }
            ast::Expr::Test(expr) => {
                visit_expr(&expr.expr, state);
                for arg in &expr.args {
                    visit_expr(arg, state);
                }
            }
            ast::Expr::GetAttr(expr) => {
                visit_expr(&expr.expr, state);
            }
            ast::Expr::GetItem(expr) => {
                visit_expr(&expr.expr, state);
                visit_expr(&expr.subscript_expr, state);
            }
            ast::Expr::Call(expr) => {
                visit_expr(&expr.expr, state);
                for arg in &expr.args {
                    visit_expr(arg, state);
                }
            }
            ast::Expr::List(expr) => {
                for value in &expr.items {
                    visit_expr(value, state);
                }
            }
            ast::Expr::Map(expr) => {
                for (key, value) in expr.keys.iter().zip(expr.values.iter()) {
                    visit_expr(key, state);
                    visit_expr(value, state);
                }
            }
        }
    }

    fn assign_nested(expr: &ast::Expr, state: &mut State) {
        match expr {
            ast::Expr::Var(var) => {
                state.assign(var.id);
            }
            ast::Expr::List(list) => {
                for expr in &list.items {
                    assign_nested(expr, state);
                }
            }
            _ => {}
        }
    }

    fn walk(node: &ast::Stmt, state: &mut State) {
        match node {
            ast::Stmt::Template(stmt) => {
                state.assign("self");
                stmt.children.iter().for_each(|x| walk(x, state));
            }
            ast::Stmt::EmitExpr(expr) => visit_expr(&expr.expr, state),
            ast::Stmt::EmitRaw(_) | ast::Stmt::Extends(_) | ast::Stmt::Include(_) => {}
            ast::Stmt::ForLoop(stmt) => {
                state.push();
                state.assign("loop");
                visit_expr(&stmt.iter, state);
                assign_nested(&stmt.target, state);
                if let Some(ref filter_expr) = stmt.filter_expr {
                    visit_expr(filter_expr, state);
                }
                stmt.body.iter().for_each(|x| walk(x, state));
                state.pop();
                state.push();
                stmt.else_body.iter().for_each(|x| walk(x, state));
                state.pop();
            }
            ast::Stmt::IfCond(stmt) => {
                visit_expr(&stmt.expr, state);
                state.push();
                stmt.true_body.iter().for_each(|x| walk(x, state));
                state.pop();
                state.push();
                stmt.false_body.iter().for_each(|x| walk(x, state));
                state.pop();
            }
            ast::Stmt::WithBlock(stmt) => {
                state.push();
                for (target, expr) in &stmt.assignments {
                    assign_nested(target, state);
                    visit_expr(expr, state);
                }
                stmt.body.iter().for_each(|x| walk(x, state));
                state.pop();
            }
            ast::Stmt::Set(stmt) => {
                assign_nested(&stmt.target, state);
                visit_expr(&stmt.expr, state);
            }
            ast::Stmt::Block(stmt) => {
                state.push();
                state.assign("super");
                stmt.body.iter().for_each(|x| walk(x, state));
                state.pop();
            }
            ast::Stmt::AutoEscape(stmt) => {
                state.push();
                stmt.body.iter().for_each(|x| walk(x, state));
                state.pop();
            }
            ast::Stmt::FilterBlock(stmt) => {
                state.push();
                stmt.body.iter().for_each(|x| walk(x, state));
                state.pop();
            }
        }
    }

    let ast = parse(source, "<string>")?;
    let mut state = State {
        out: HashSet::new(),
        assigned: vec![Default::default()],
    };
    walk(&ast, &mut state);
    Ok(state.out)
}

/// Given a template source returns a set of referenced templates by name.
///
/// This parses the given template and returns a hash set of all referenced
/// templates.  These are templates referenced by `{% extends %}` and
/// `{% include %}`.  Note that since both of those blocks support variables
/// this function cannot always return all templates.  If a variable or
/// complex expression is used to reference a template then the string
/// `"*"` is also included.
///
/// # Example
///
/// ```rust
/// # use minijinja::meta::find_referenced_templates;
/// let names = find_referenced_templates(r#"
///     {% extends "layout.html" %}{% include helper %}
/// "#).unwrap();
/// assert!(names.contains("layout.html"));
/// assert!(names.contains("*"));
/// ```
pub fn find_referenced_templates(source: &str) -> Result<HashSet<String>, Error> {
    fn record_reference(expr: &ast::Expr, out: &mut HashSet<String>) {
        if let ast::Expr::Const(val) = expr {
            if let Some(s) = val.value.as_str() {
                out.insert(s.to_string());
                return;
            }
        }
        out.insert("*".into());
    }

    fn walk(node: &ast::Stmt, out: &mut HashSet<String>) {
        match node {
            ast::Stmt::Template(stmt) => stmt.children.iter().for_each(|x| walk(x, out)),
            ast::Stmt::EmitExpr(_) | ast::Stmt::EmitRaw(_) | ast::Stmt::Set(_) => {}
            ast::Stmt::ForLoop(stmt) => stmt
                .body
                .iter()
                .chain(stmt.else_body.iter())
                .for_each(|x| walk(x, out)),
            ast::Stmt::IfCond(stmt) => stmt
                .true_body
                .iter()
                .chain(stmt.false_body.iter())
                .for_each(|x| walk(x, out)),
            ast::Stmt::WithBlock(stmt) => stmt.body.iter().for_each(|x| walk(x, out)),
            ast::Stmt::Block(stmt) => stmt.body.iter().for_each(|x| walk(x, out)),
            ast::Stmt::Extends(stmt) => record_reference(&stmt.name, out),
            ast::Stmt::Include(stmt) => record_reference(&stmt.name, out),
            ast::Stmt::AutoEscape(stmt) => stmt.body.iter().for_each(|x| walk(x, out)),
            ast::Stmt::FilterBlock(stmt) => stmt.body.iter().for_each(|x| walk(x, out)),
        }
    }

    let ast = parse(source, "<string>")?;
    let mut rv = HashSet::new();
    walk(&ast, &mut rv);
    Ok(rv)
}

#[test]
fn test_find_undeclared_variables() {
    let names = find_undeclared_variables(
        r#"{% with foo = 42 %}{{ foo }} {{ bar }} {{ bar(baz) }}{% endwith %}"#,
    )
    .unwrap();
    assert_eq!(names, {
        let mut s = HashSet::new();
        s.insert("bar".to_string());
        s.insert("baz".to_string());
        s
    });
}

#[test]
fn test_find_referenced_templates() {
    let names =
        find_referenced_templates(r#"{% extends "layout.html" %}{% include helper %}"#).unwrap();
    assert_eq!(names, {
        let mut s = HashSet::new();
        s.insert("*".to_string());
        s.insert("layout.html".to_string());
        s
    });
}
