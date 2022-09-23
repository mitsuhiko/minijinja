use std::collections::HashSet;

use crate::compiler::ast;

struct AssignmentTracker<'a> {
    out: HashSet<&'a str>,
    assigned: Vec<HashSet<&'a str>>,
}

impl<'a> AssignmentTracker<'a> {
    fn is_assigned(&self, name: &str) -> bool {
        self.assigned.iter().any(|x| x.contains(name))
    }

    fn assign(&mut self, name: &'a str) {
        self.assigned.last_mut().unwrap().insert(name);
    }

    fn push(&mut self) {
        self.assigned.push(Default::default());
    }

    fn pop(&mut self) {
        self.assigned.pop();
    }
}

/// Finds all variables that need to be captured as closure for a macro.
pub fn find_macro_closure<'a>(m: &ast::Macro<'a>) -> HashSet<&'a str> {
    fn visit_expr<'a>(expr: &ast::Expr<'a>, state: &mut AssignmentTracker<'a>) {
        match expr {
            ast::Expr::Var(var) => {
                if !state.is_assigned(var.id) {
                    state.out.insert(var.id);
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
            ast::Expr::Slice(slice) => {
                if let Some(ref start) = slice.start {
                    visit_expr(start, state);
                }
                if let Some(ref stop) = slice.stop {
                    visit_expr(stop, state);
                }
                if let Some(ref step) = slice.step {
                    visit_expr(step, state);
                }
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
            ast::Expr::Kwargs(expr) => {
                for (_, value) in &expr.pairs {
                    visit_expr(value, state);
                }
            }
        }
    }

    fn assign_nested<'a>(expr: &ast::Expr<'a>, state: &mut AssignmentTracker<'a>) {
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

    fn walk<'a>(node: &ast::Stmt<'a>, state: &mut AssignmentTracker<'a>) {
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
            ast::Stmt::SetBlock(stmt) => {
                assign_nested(&stmt.target, state);
                state.push();
                stmt.body.iter().for_each(|x| walk(x, state));
                state.pop();
            }
            ast::Stmt::Macro(stmt) => {
                state.assign(stmt.name);
            }
            ast::Stmt::Import(stmt) => {
                assign_nested(&stmt.name, state);
            }
            ast::Stmt::FromImport(stmt) => {
                for (arg, alias) in &stmt.names {
                    assign_nested(alias.as_ref().unwrap_or(arg), state);
                }
            }
        }
    }

    let mut state = AssignmentTracker {
        out: HashSet::new(),
        assigned: vec![Default::default()],
    };

    for arg in &m.args {
        assign_nested(arg, &mut state);
    }

    for node in &m.body {
        walk(node, &mut state);
    }

    state.out
}
