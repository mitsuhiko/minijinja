use std::fmt::Write;

use crate::compiler::ast::CallArg;

use super::ast::{BinOpKind, Expr, Stmt, Template};

pub trait ToSource {
    fn to_source(&self, f: &mut String, indent: usize) -> std::fmt::Result;
}

impl<'a> ToSource for Stmt<'a> {
    fn to_source(&self, f: &mut String, indent: usize) -> std::fmt::Result {
        match self {
            Stmt::Template(t) => t.to_source(f, indent),
            Stmt::EmitExpr(e) => {
                write!(f, "{{{{ ")?;
                e.expr.to_source(f, 0)?;
                write!(f, " }}}}")
            }
            Stmt::EmitRaw(r) => write!(f, "{}", r.raw),
            Stmt::ForLoop(fl) => {
                write!(f, "{:indent$}{{% for ", "", indent = 0)?;
                fl.target.to_source(f, 0)?;
                write!(f, " in ")?;
                fl.iter.to_source(f, 0)?;
                if let Some(filter) = &fl.filter_expr {
                    write!(f, " if ")?;
                    filter.to_source(f, 0)?;
                }
                writeln!(f, " %}}")?;

                for stmt in &fl.body {
                    stmt.to_source(f, indent + 2)?;
                    writeln!(f)?;
                }

                if !fl.else_body.is_empty() {
                    writeln!(f, "{:indent$}{{% else %}}", "", indent = 0)?;
                    for stmt in &fl.else_body {
                        stmt.to_source(f, indent + 2)?;
                        writeln!(f)?;
                    }
                }

                write!(f, "{:indent$}{{% endfor %}}", "", indent = 0)
            }
            // Add other statement types...
            _ => Ok(()),
        }
    }
}

impl<'a> ToSource for Expr<'a> {
    fn to_source(&self, f: &mut String, _indent: usize) -> std::fmt::Result {
        match self {
            Expr::Var(v) => write!(f, "{}", v.id),
            Expr::Const(c) => write!(f, "{}", c.value),
            Expr::BinOp(b) => {
                b.left.to_source(f, 0)?;
                write!(
                    f,
                    " {} ",
                    match b.op {
                        BinOpKind::Add => "+",
                        BinOpKind::Sub => "-",
                        BinOpKind::Mul => "*",
                        BinOpKind::Div => "/",
                        BinOpKind::Eq => "==",
                        BinOpKind::Ne => "!=",
                        BinOpKind::Lt => "<",
                        BinOpKind::Lte => "<=",
                        BinOpKind::Gt => ">",
                        BinOpKind::Gte => ">=",
                        BinOpKind::ScAnd => "and",
                        BinOpKind::ScOr => "or",
                        BinOpKind::FloorDiv => "//",
                        BinOpKind::Rem => "%",
                        BinOpKind::Pow => "**",
                        BinOpKind::Concat => "~",
                        BinOpKind::In => "in",
                    }
                )?;
                b.right.to_source(f, 0)
            }
            Expr::Call(c) => {
                c.expr.to_source(f, 0)?;
                write!(f, "(")?;
                for (i, arg) in c.args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    match arg {
                        CallArg::Pos(e) => e.to_source(f, 0)?,
                        CallArg::Kwarg(name, e) => {
                            write!(f, "{}=", name)?;
                            e.to_source(f, 0)?;
                        }
                        // Handle other arg types...
                        _ => {}
                    }
                }
                write!(f, ")")
            }
            Expr::IfExpr(if_expr) => {
                // Write the true expression
                if_expr.true_expr.to_source(f, 0)?;

                // Write the if condition
                write!(f, " if ")?;
                if_expr.test_expr.to_source(f, 0)?;

                // Write the else expression if it exists
                if let Some(false_expr) = &if_expr.false_expr {
                    write!(f, " else ")?;
                    false_expr.to_source(f, 0)?;
                }

                Ok(())
            }
            // Add other expression types...
            _ => Ok(()),
        }
    }
}

impl<'a> ToSource for Template<'a> {
    fn to_source(&self, f: &mut String, indent: usize) -> std::fmt::Result {
        for stmt in &self.children {
            stmt.to_source(f, indent)?;
            writeln!(f)?;
        }
        Ok(())
    }
}
