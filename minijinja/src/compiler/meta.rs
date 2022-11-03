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
    fn visit_expr_opt<'a>(
        m: &ast::Macro<'a>,
        expr: &Option<ast::Expr<'a>>,
        state: &mut AssignmentTracker<'a>
    ) {
        if let Some(expr) = expr {
            visit_expr(m, expr, state);
        }
    }

    fn visit_expr<'a>(
        m: &ast::Macro<'a>,
        expr: &ast::Expr<'a>,
        state: &mut AssignmentTracker<'a>
    ) {
        match expr {
            ast::Expr::Var(var) => {
                if !state.is_assigned(var.id) {
                    state.out.insert(var.id);
                    state.assign(var.id);
                }
            }
            ast::Expr::Const(_) => {}
            ast::Expr::UnaryOp(expr) => visit_expr(m, &expr.expr, state),
            ast::Expr::BinOp(expr) => {
                visit_expr(m, &expr.left, state);
                visit_expr(m, &expr.right, state);
            }
            ast::Expr::IfExpr(expr) => {
                visit_expr(m, &expr.test_expr, state);
                visit_expr(m, &expr.true_expr, state);
                visit_expr_opt(m, &expr.false_expr, state);
            }
            ast::Expr::Filter(expr) => {
                visit_expr_opt(m, &expr.expr, state);
                expr.args.iter().for_each(|x| visit_expr(m, x, state));
            }
            ast::Expr::Test(expr) => {
                visit_expr(m, &expr.expr, state);
                expr.args.iter().for_each(|x| visit_expr(m, x, state));
            }
            ast::Expr::GetAttr(expr) => visit_expr(m, &expr.expr, state),
            ast::Expr::GetItem(expr) => {
                visit_expr(m, &expr.expr, state);
                visit_expr(m, &expr.subscript_expr, state);
            }
            ast::Expr::Slice(slice) => {
                visit_expr_opt(m, &slice.start, state);
                visit_expr_opt(m, &slice.stop, state);
                visit_expr_opt(m, &slice.step, state);
            }
            ast::Expr::Call(expr) => {
                visit_expr(m, &expr.expr, state);
                expr.args.iter().for_each(|x| visit_expr(m, x, state));
            }
            ast::Expr::List(expr) => expr.items.iter().for_each(|x| visit_expr(m, x, state)),
            ast::Expr::Map(expr) => expr.keys.iter().zip(expr.values.iter()).for_each(|(k, v)| {
                visit_expr(m, k, state);
                visit_expr(m, v, state);
            }),
            ast::Expr::Kwargs(expr) => expr.pairs.iter().for_each(|(_, v)| visit_expr(m, v, state)),
        }
    }

    fn assign_nested<'a>(
        m: &ast::Macro<'a>,
        expr: &ast::Expr<'a>,
        state: &mut AssignmentTracker<'a>
    ) {
        match expr {
            ast::Expr::Var(var) => state.assign(var.id),
            ast::Expr::List(list) => list.items.iter().for_each(|x| assign_nested(m, x, state)),
            _ => {}
        }
    }

    fn walk<'a>(
        m: &ast::Macro<'a>,
        node: &ast::Stmt<'a>,
        state: &mut AssignmentTracker<'a>
    ) {
        match node {
            ast::Stmt::Template(stmt) => {
                state.assign("self");
                stmt.children.iter().for_each(|x| walk(m, x, state));
            }
            ast::Stmt::EmitExpr(expr) => visit_expr(m, &expr.expr, state),
            ast::Stmt::EmitRaw(_) => {}
            ast::Stmt::ForLoop(stmt) => {
                state.push();
                state.assign("loop");
                visit_expr(m, &stmt.iter, state);
                assign_nested(m, &stmt.target, state);
                visit_expr_opt(m, &stmt.filter_expr, state);
                stmt.body.iter().for_each(|x| walk(m, x, state));
                state.pop();
                state.push();
                stmt.else_body.iter().for_each(|x| walk(m, x, state));
                state.pop();
            }
            ast::Stmt::IfCond(stmt) => {
                visit_expr(m, &stmt.expr, state);
                state.push();
                stmt.true_body.iter().for_each(|x| walk(m, x, state));
                state.pop();
                state.push();
                stmt.false_body.iter().for_each(|x| walk(m, x, state));
                state.pop();
            }
            ast::Stmt::WithBlock(stmt) => {
                state.push();
                for (target, expr) in &stmt.assignments {
                    assign_nested(m, target, state);
                    visit_expr(m, expr, state);
                }
                stmt.body.iter().for_each(|x| walk(m, x, state));
                state.pop();
            }
            ast::Stmt::Set(stmt) => {
                assign_nested(m, &stmt.target, state);
                visit_expr(m, &stmt.expr, state);
            }
            ast::Stmt::Block(stmt) => {
                state.push();
                state.assign("super");
                stmt.body.iter().for_each(|x| walk(m, x, state));
                state.pop();
            }
            ast::Stmt::AutoEscape(stmt) => {
                state.push();
                stmt.body.iter().for_each(|x| walk(m, x, state));
                state.pop();
            }
            ast::Stmt::FilterBlock(stmt) => {
                state.push();
                stmt.body.iter().for_each(|x| walk(m, x, state));
                state.pop();
            }
            ast::Stmt::SetBlock(stmt) => {
                assign_nested(m, &stmt.target, state);
                state.push();
                stmt.body.iter().for_each(|x| walk(m, x, state));
                state.pop();
            }
            #[cfg(feature = "multi-template")]
            ast::Stmt::Extends(_) | ast::Stmt::Include(_) => {}
            #[cfg(feature = "multi-template")]
            ast::Stmt::Import(stmt) => {
                assign_nested(m, &stmt.name, state);
            }
            #[cfg(feature = "multi-template")]
            ast::Stmt::FromImport(stmt) => stmt.names.iter().for_each(|(arg, alias)| {
                assign_nested(m, alias.as_ref().unwrap_or(arg), state);
            }),
            #[cfg(feature = "macros")]
            ast::Stmt::Macro(stmt) => {
                state.assign(stmt.name);
            }
        }
    }

    let mut state = AssignmentTracker {
        out: HashSet::new(),
        assigned: vec![Default::default()],
    };

    m.args.iter().for_each(|arg| assign_nested(m, arg, &mut state));
    m.body.iter().for_each(|node| walk(m, node, &mut state));

    state.out
}
