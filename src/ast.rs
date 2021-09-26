use std::fmt;
use std::ops::Deref;

use crate::tokens::Span;
use crate::value::Value;

/// Container for nodes with location info.
///
/// This container fulfills two purposes: it adds location information
/// to nodes, but it also ensures the nodes is heap allocated.  The
/// latter is useful to ensure that enum variants do not cause the enum
/// to become too large.
#[derive(Clone)]
pub struct Spanned<T> {
    node: Box<T>,
    span: Span,
}

impl<T> Spanned<T> {
    /// Creates a new spanned node.
    pub fn new(node: T, span: Span) -> Spanned<T> {
        Spanned {
            node: Box::new(node),
            span,
        }
    }

    /// Accesses the span.
    pub fn span(&self) -> Span {
        self.span
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

impl<T: fmt::Debug> fmt::Debug for Spanned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.node, f)?;
        write!(f, "{:?}", self.span)
    }
}

/// A statement node.
#[derive(Clone)]
pub enum Stmt<'a> {
    Template(Spanned<Template<'a>>),
    EmitExpr(Spanned<EmitExpr<'a>>),
    EmitRaw(Spanned<EmitRaw<'a>>),
    ForLoop(Spanned<ForLoop<'a>>),
    IfCond(Spanned<IfCond<'a>>),
    WithBlock(Spanned<WithBlock<'a>>),
    Block(Spanned<Block<'a>>),
    Extends(Spanned<Extends<'a>>),
    Include(Spanned<Include<'a>>),
    AutoEscape(Spanned<AutoEscape<'a>>),
}

impl<'a> fmt::Debug for Stmt<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stmt::Template(s) => fmt::Debug::fmt(s, f),
            Stmt::EmitExpr(s) => fmt::Debug::fmt(s, f),
            Stmt::EmitRaw(s) => fmt::Debug::fmt(s, f),
            Stmt::ForLoop(s) => fmt::Debug::fmt(s, f),
            Stmt::IfCond(s) => fmt::Debug::fmt(s, f),
            Stmt::WithBlock(s) => fmt::Debug::fmt(s, f),
            Stmt::Block(s) => fmt::Debug::fmt(s, f),
            Stmt::Extends(s) => fmt::Debug::fmt(s, f),
            Stmt::Include(s) => fmt::Debug::fmt(s, f),
            Stmt::AutoEscape(s) => fmt::Debug::fmt(s, f),
        }
    }
}

/// An expression node.
#[derive(Clone)]
pub enum Expr<'a> {
    Var(Spanned<Var<'a>>),
    Const(Spanned<Const>),
    UnaryOp(Spanned<UnaryOp<'a>>),
    BinOp(Spanned<BinOp<'a>>),
    Filter(Spanned<Filter<'a>>),
    Test(Spanned<Test<'a>>),
    GetAttr(Spanned<GetAttr<'a>>),
    GetItem(Spanned<GetItem<'a>>),
    Call(Spanned<Call<'a>>),
    List(Spanned<List<'a>>),
    Map(Spanned<Map<'a>>),
}

impl<'a> fmt::Debug for Expr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Var(s) => fmt::Debug::fmt(s, f),
            Expr::Const(s) => fmt::Debug::fmt(s, f),
            Expr::UnaryOp(s) => fmt::Debug::fmt(s, f),
            Expr::BinOp(s) => fmt::Debug::fmt(s, f),
            Expr::Filter(s) => fmt::Debug::fmt(s, f),
            Expr::Test(s) => fmt::Debug::fmt(s, f),
            Expr::GetAttr(s) => fmt::Debug::fmt(s, f),
            Expr::GetItem(s) => fmt::Debug::fmt(s, f),
            Expr::Call(s) => fmt::Debug::fmt(s, f),
            Expr::List(s) => fmt::Debug::fmt(s, f),
            Expr::Map(s) => fmt::Debug::fmt(s, f),
        }
    }
}

/// Root template node.
#[derive(Debug, Clone)]
pub struct Template<'a> {
    pub children: Vec<Stmt<'a>>,
}

/// A for loop.
#[derive(Debug, Clone)]
pub struct ForLoop<'a> {
    pub target: Expr<'a>,
    pub iter: Expr<'a>,
    pub filter_expr: Option<Expr<'a>>,
    pub body: Vec<Stmt<'a>>,
    pub else_body: Vec<Stmt<'a>>,
}

/// An if/else condition.
#[derive(Debug, Clone)]
pub struct IfCond<'a> {
    pub expr: Expr<'a>,
    pub true_body: Vec<Stmt<'a>>,
    pub false_body: Vec<Stmt<'a>>,
}

/// A with block.
#[derive(Debug, Clone)]
pub struct WithBlock<'a> {
    pub assignments: Vec<(&'a str, Expr<'a>)>,
    pub body: Vec<Stmt<'a>>,
}

/// A block for inheritance elements.
#[derive(Debug, Clone)]
pub struct Block<'a> {
    pub name: &'a str,
    pub body: Vec<Stmt<'a>>,
}

/// An extends block.
#[derive(Debug, Clone)]
pub struct Extends<'a> {
    pub name: Expr<'a>,
}

/// An include block.
#[derive(Debug, Clone)]
pub struct Include<'a> {
    pub name: Expr<'a>,
}

/// An auto escape control block.
#[derive(Debug, Clone)]
pub struct AutoEscape<'a> {
    pub enabled: Expr<'a>,
    pub body: Vec<Stmt<'a>>,
}

/// Outputs the expression.
#[derive(Debug, Clone)]
pub struct EmitExpr<'a> {
    pub expr: Expr<'a>,
}

/// Outputs raw template code.
#[derive(Debug, Clone)]
pub struct EmitRaw<'a> {
    pub raw: &'a str,
}

/// Looks up a variable.
#[derive(Debug, Clone)]
pub struct Var<'a> {
    pub id: &'a str,
}

/// Loads a constant
#[derive(Debug, Clone)]
pub struct Const {
    pub value: Value,
}

/// A kind of unary operator.
#[derive(Debug, Clone)]
pub enum UnaryOpKind {
    Not,
    Neg,
}

/// An unary operator expression.
#[derive(Debug, Clone)]
pub struct UnaryOp<'a> {
    pub op: UnaryOpKind,
    pub expr: Expr<'a>,
}

/// A kind of binary operator.
#[derive(Debug, Clone)]
pub enum BinOpKind {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    ScAnd,
    ScOr,
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Rem,
    Pow,
    Concat,
    In,
}

/// A binary operator expression.
#[derive(Debug, Clone)]
pub struct BinOp<'a> {
    pub op: BinOpKind,
    pub left: Expr<'a>,
    pub right: Expr<'a>,
}

/// A filter expression.
#[derive(Debug, Clone)]
pub struct Filter<'a> {
    pub name: &'a str,
    pub expr: Expr<'a>,
    pub args: Vec<Expr<'a>>,
}

/// A test expression.
#[derive(Debug, Clone)]
pub struct Test<'a> {
    pub name: &'a str,
    pub expr: Expr<'a>,
    pub args: Vec<Expr<'a>>,
}

/// An attribute lookup expression.
#[derive(Debug, Clone)]
pub struct GetAttr<'a> {
    pub expr: Expr<'a>,
    pub name: &'a str,
}

/// An item lookup expression.
#[derive(Debug, Clone)]
pub struct GetItem<'a> {
    pub expr: Expr<'a>,
    pub subscript_expr: Expr<'a>,
}

/// Calls something.
#[derive(Debug, Clone)]
pub struct Call<'a> {
    pub expr: Expr<'a>,
    pub args: Vec<Expr<'a>>,
}

/// Creates a list of values.
#[derive(Debug, Clone)]
pub struct List<'a> {
    pub items: Vec<Expr<'a>>,
}

/// Creates a map of values.
#[derive(Debug, Clone)]
pub struct Map<'a> {
    pub keys: Vec<Expr<'a>>,
    pub values: Vec<Expr<'a>>,
}

#[derive(Debug)]
pub enum CallType<'ast, 'source> {
    Function(&'source str),
    Method(&'ast Expr<'source>, &'source str),
    Object(&'ast Expr<'source>),
}

impl<'a> Call<'a> {
    /// Try to isolate a method call.
    ///
    /// name + call and attribute lookup + call are really method
    /// calls which are easier to handle for the compiler as a separate
    /// thing.
    pub fn identify_call(&self) -> CallType<'_, 'a> {
        match self.expr {
            Expr::Var(ref var) => CallType::Function(var.id),
            Expr::GetAttr(ref attr) => CallType::Method(&attr.expr, attr.name),
            _ => CallType::Object(&self.expr),
        }
    }
}
