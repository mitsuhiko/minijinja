use std::fmt;

use crate::compiler::ast::{self, Spanned};
use crate::compiler::lexer::tokenize;
use crate::compiler::tokens::{Span, Token};
use crate::error::{Error, ErrorKind};
use crate::value::Value;

const RESERVED_NAMES: [&str; 8] = [
    "true", "True", "false", "False", "none", "None", "loop", "self",
];

macro_rules! syntax_error {
    ($msg:expr) => {{
        return Err(Error::new(ErrorKind::SyntaxError, $msg));
    }};
    ($msg:expr, $($tt:tt)*) => {{
        return Err(Error::new(ErrorKind::SyntaxError, format!($msg, $($tt)*)));
    }};
}

fn unexpected<D: fmt::Display>(unexpected: D, expected: &str) -> Error {
    Error::new(
        ErrorKind::SyntaxError,
        format!("unexpected {}, expected {}", unexpected, expected),
    )
}

fn unexpected_eof(expected: &str) -> Error {
    unexpected("end of input", expected)
}

fn make_const(value: Value, span: Span) -> ast::Expr<'static> {
    ast::Expr::Const(Spanned::new(ast::Const { value }, span))
}

macro_rules! expect_token {
    ($parser:expr, $expectation:expr) => {{
        match $parser.stream.next()? {
            Some(rv) => Ok(rv),
            None => Err(unexpected_eof($expectation)),
        }
    }};
    ($parser:expr, $match:pat, $expectation:expr) => {{
        match $parser.stream.next()? {
            Some((token, span)) if matches!(token, $match) => Ok((token, span)),
            Some((token, _)) => Err(unexpected(token, $expectation)),
            None => Err(unexpected_eof($expectation)),
        }
    }};
    ($parser:expr, $match:pat => $target:expr, $expectation:expr) => {{
        match $parser.stream.next()? {
            Some(($match, span)) => Ok(($target, span)),
            Some((token, _)) => Err(unexpected(token, $expectation)),
            None => Err(unexpected_eof($expectation)),
        }
    }};
}

enum SetParseResult<'a> {
    Set(ast::Set<'a>),
    SetBlock(ast::SetBlock<'a>),
}

struct TokenStream<'a> {
    iter: Box<dyn Iterator<Item = Result<(Token<'a>, Span), Error>> + 'a>,
    current: Option<Result<(Token<'a>, Span), Error>>,
    last_span: Span,
}

impl<'a> TokenStream<'a> {
    /// Tokenize a template
    pub fn new(source: &'a str, in_expr: bool) -> TokenStream<'a> {
        TokenStream {
            iter: (Box::new(tokenize(source, in_expr)) as Box<dyn Iterator<Item = _>>),
            current: None,
            last_span: Span::default(),
        }
    }

    /// Advance the stream.
    #[inline(always)]
    pub fn next(&mut self) -> Result<Option<(Token<'a>, Span)>, Error> {
        let rv = self.current.take();
        self.current = self.iter.next();
        if let Some(Ok((_, span))) = rv {
            self.last_span = span;
        }
        rv.transpose()
    }

    /// Look at the current token
    #[inline(always)]
    pub fn current(&mut self) -> Result<Option<(&Token<'a>, Span)>, Error> {
        if self.current.is_none() {
            self.next()?;
        }
        match self.current {
            Some(Ok(ref tok)) => Ok(Some((&tok.0, tok.1))),
            Some(Err(_)) => Err(self.current.take().unwrap().unwrap_err()),
            None => Ok(None),
        }
    }

    /// Expands the span
    #[inline(always)]
    pub fn expand_span(&self, mut span: Span) -> Span {
        span.end_line = self.last_span.end_line;
        span.end_col = self.last_span.end_col;
        span
    }

    /// Returns the current span.
    #[inline(always)]
    pub fn current_span(&self) -> Span {
        if let Some(Ok((_, span))) = self.current {
            span
        } else {
            self.last_span
        }
    }

    /// Returns the last seen span.
    #[inline(always)]
    pub fn last_span(&self) -> Span {
        self.last_span
    }
}

struct Parser<'a> {
    stream: TokenStream<'a>,
}

macro_rules! binop {
    ($func:ident, $next:ident, { $($tok:tt)* }) => {
        fn $func(&mut self) -> Result<ast::Expr<'a>, Error> {
            let span = self.stream.current_span();
            let mut left = self.$next()?;
            loop {
                let op = match self.stream.current()? {
                    $($tok)*
                    _ => break,
                };
                self.stream.next()?;
                let right = self.$next()?;
                left = ast::Expr::BinOp(Spanned::new(
                    ast::BinOp {
                        op,
                        left,
                        right,
                    },
                    self.stream.expand_span(span),
                ));
            }
            Ok(left)
        }
    };
}

macro_rules! unaryop {
    ($func:ident, $next:ident, { $($tok:tt)* }) => {
        fn $func(&mut self) -> Result<ast::Expr<'a>, Error> {
            let span = self.stream.current_span();
            let op = match self.stream.current()? {
                $($tok)*
                _ => return self.$next()
            };
            self.stream.next()?;
            Ok(ast::Expr::UnaryOp(Spanned::new(
                ast::UnaryOp {
                    op,
                    expr: self.$func()?,
                },
                self.stream.expand_span(span),
            )))
        }
    };
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, in_expr: bool) -> Parser<'a> {
        Parser {
            stream: TokenStream::new(source, in_expr),
        }
    }

    fn parse_ifexpr(&mut self) -> Result<ast::Expr<'a>, Error> {
        let mut span = self.stream.last_span();
        let mut expr = self.parse_or()?;
        loop {
            if matches!(self.stream.current()?, Some((Token::Ident("if"), _))) {
                self.stream.next()?;
                let expr2 = self.parse_or()?;
                let expr3 = if matches!(self.stream.current()?, Some((Token::Ident("else"), _))) {
                    self.stream.next()?;
                    Some(self.parse_ifexpr()?)
                } else {
                    None
                };
                expr = ast::Expr::IfExpr(Spanned::new(
                    ast::IfExpr {
                        test_expr: expr2,
                        true_expr: expr,
                        false_expr: expr3,
                    },
                    self.stream.expand_span(span),
                ));
                span = self.stream.last_span();
            } else {
                break;
            }
        }
        Ok(expr)
    }

    binop!(parse_or, parse_and, {
        Some((Token::Ident("or"), _)) => ast::BinOpKind::ScOr,
    });
    binop!(parse_and, parse_not, {
        Some((Token::Ident("and"), _)) => ast::BinOpKind::ScAnd,
    });
    unaryop!(parse_not, parse_compare, {
        Some((Token::Ident("not"), _)) => ast::UnaryOpKind::Not,
    });

    fn parse_compare(&mut self) -> Result<ast::Expr<'a>, Error> {
        let mut span = self.stream.last_span();
        let mut expr = self.parse_math1()?;
        loop {
            let mut negated = false;
            let op = match self.stream.current()? {
                Some((Token::Eq, _)) => ast::BinOpKind::Eq,
                Some((Token::Ne, _)) => ast::BinOpKind::Ne,
                Some((Token::Lt, _)) => ast::BinOpKind::Lt,
                Some((Token::Lte, _)) => ast::BinOpKind::Lte,
                Some((Token::Gt, _)) => ast::BinOpKind::Gt,
                Some((Token::Gte, _)) => ast::BinOpKind::Gte,
                Some((Token::Ident("in"), _)) => ast::BinOpKind::In,
                Some((Token::Ident("not"), _)) => {
                    self.stream.next()?;
                    expect_token!(self, Token::Ident("in"), "in")?;
                    negated = true;
                    ast::BinOpKind::In
                }
                _ => break,
            };
            if !negated {
                self.stream.next()?;
            }
            expr = ast::Expr::BinOp(Spanned::new(
                ast::BinOp {
                    op,
                    left: expr,
                    right: self.parse_math1()?,
                },
                self.stream.expand_span(span),
            ));
            if negated {
                expr = ast::Expr::UnaryOp(Spanned::new(
                    ast::UnaryOp {
                        op: ast::UnaryOpKind::Not,
                        expr,
                    },
                    self.stream.expand_span(span),
                ));
            }
            span = self.stream.last_span();
        }
        Ok(expr)
    }

    binop!(parse_math1, parse_concat, {
        Some((Token::Plus, _)) => ast::BinOpKind::Add,
        Some((Token::Minus, _)) => ast::BinOpKind::Sub,
    });
    binop!(parse_concat, parse_math2, {
        Some((Token::Tilde, _)) => ast::BinOpKind::Concat,
    });
    binop!(parse_math2, parse_pow, {
        Some((Token::Mul, _)) => ast::BinOpKind::Mul,
        Some((Token::Div, _)) => ast::BinOpKind::Div,
        Some((Token::FloorDiv, _)) => ast::BinOpKind::FloorDiv,
        Some((Token::Mod, _)) => ast::BinOpKind::Rem,
    });
    binop!(parse_pow, parse_unary, {
        Some((Token::Pow, _)) => ast::BinOpKind::Pow,
    });
    unaryop!(parse_unary_only, parse_primary, {
        Some((Token::Minus, _)) => ast::UnaryOpKind::Neg,
    });

    fn parse_unary(&mut self) -> Result<ast::Expr<'a>, Error> {
        let span = self.stream.current_span();
        let mut expr = self.parse_unary_only()?;
        expr = self.parse_postfix(expr, span)?;
        self.parse_filter_expr(expr)
    }

    fn parse_postfix(
        &mut self,
        expr: ast::Expr<'a>,
        mut span: Span,
    ) -> Result<ast::Expr<'a>, Error> {
        let mut expr = expr;
        loop {
            let next_span = self.stream.current_span();
            match self.stream.current()? {
                Some((Token::Dot, _)) => {
                    self.stream.next()?;
                    let (name, _) = expect_token!(self, Token::Ident(name) => name, "identifier")?;
                    expr = ast::Expr::GetAttr(Spanned::new(
                        ast::GetAttr { name, expr },
                        self.stream.expand_span(span),
                    ));
                }
                Some((Token::BracketOpen, _)) => {
                    self.stream.next()?;

                    let mut start = None;
                    let mut stop = None;
                    let mut step = None;
                    let mut is_slice = false;

                    if !matches!(self.stream.current()?, Some((Token::Colon, _))) {
                        start = Some(self.parse_expr()?);
                    }
                    if matches!(self.stream.current()?, Some((Token::Colon, _))) {
                        is_slice = true;
                        self.stream.next()?;
                        if !matches!(
                            self.stream.current()?,
                            Some((Token::BracketClose | Token::Colon, _))
                        ) {
                            stop = Some(self.parse_expr()?);
                        }
                        if matches!(self.stream.current()?, Some((Token::Colon, _))) {
                            self.stream.next()?;
                            if !matches!(self.stream.current()?, Some((Token::BracketClose, _))) {
                                step = Some(self.parse_expr()?);
                            }
                        }
                    }
                    expect_token!(self, Token::BracketClose, "`]`")?;

                    if !is_slice {
                        expr = ast::Expr::GetItem(Spanned::new(
                            ast::GetItem {
                                expr,
                                subscript_expr: start.ok_or_else(|| {
                                    Error::new(ErrorKind::SyntaxError, "empty subscript")
                                })?,
                            },
                            self.stream.expand_span(span),
                        ));
                    } else {
                        expr = ast::Expr::Slice(Spanned::new(
                            ast::Slice {
                                expr,
                                start,
                                stop,
                                step,
                            },
                            self.stream.expand_span(span),
                        ));
                    }
                }
                Some((Token::ParenOpen, _)) => {
                    let args = self.parse_args()?;
                    expr = ast::Expr::Call(Spanned::new(
                        ast::Call { expr, args },
                        self.stream.expand_span(span),
                    ));
                }
                _ => break,
            }
            span = next_span;
        }
        Ok(expr)
    }

    fn parse_filter_expr(&mut self, expr: ast::Expr<'a>) -> Result<ast::Expr<'a>, Error> {
        let mut expr = expr;
        loop {
            match self.stream.current()? {
                Some((Token::Pipe, _)) => {
                    self.stream.next()?;
                    let (name, span) =
                        expect_token!(self, Token::Ident(name) => name, "identifier")?;
                    let args = if matches!(self.stream.current()?, Some((Token::ParenOpen, _))) {
                        self.parse_args()?
                    } else {
                        Vec::new()
                    };
                    expr = ast::Expr::Filter(Spanned::new(
                        ast::Filter {
                            name,
                            expr: Some(expr),
                            args,
                        },
                        self.stream.expand_span(span),
                    ));
                }
                Some((Token::Ident("is"), _)) => {
                    self.stream.next()?;
                    let negated =
                        if matches!(self.stream.current()?, Some((Token::Ident("not"), _))) {
                            self.stream.next()?;
                            true
                        } else {
                            false
                        };
                    let (name, span) =
                        expect_token!(self, Token::Ident(name) => name, "identifier")?;
                    let args = if matches!(self.stream.current()?, Some((Token::ParenOpen, _))) {
                        self.parse_args()?
                    } else {
                        Vec::new()
                    };
                    expr = ast::Expr::Test(Spanned::new(
                        ast::Test { name, expr, args },
                        self.stream.expand_span(span),
                    ));
                    if negated {
                        expr = ast::Expr::UnaryOp(Spanned::new(
                            ast::UnaryOp {
                                op: ast::UnaryOpKind::Not,
                                expr,
                            },
                            self.stream.expand_span(span),
                        ));
                    }
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_args(&mut self) -> Result<Vec<ast::Expr<'a>>, Error> {
        let mut args = Vec::new();
        let mut first_span = None;
        let mut kwargs_keys = Vec::new();
        let mut kwargs_values = Vec::new();

        expect_token!(self, Token::ParenOpen, "`(`")?;
        loop {
            if matches!(self.stream.current()?, Some((Token::ParenClose, _))) {
                break;
            }
            if !args.is_empty() || !kwargs_keys.is_empty() {
                expect_token!(self, Token::Comma, "`,`")?;
            }
            if matches!(self.stream.current()?, Some((Token::ParenClose, _))) {
                break;
            }
            let expr = self.parse_expr()?;

            // keyword argument
            match expr {
                ast::Expr::Var(ref var)
                    if matches!(self.stream.current()?, Some((Token::Assign, _))) =>
                {
                    self.stream.next()?;
                    if first_span.is_none() {
                        first_span = Some(var.span());
                    }
                    kwargs_keys.push(ast::Expr::Const(Spanned::new(
                        ast::Const {
                            value: Value::from(var.id),
                        },
                        var.span(),
                    )));
                    kwargs_values.push(self.parse_expr_noif()?);
                }
                _ if !kwargs_keys.is_empty() => {
                    return Err(Error::new(
                        ErrorKind::SyntaxError,
                        "non-keyword arg after keyword arg",
                    ));
                }
                _ => {
                    args.push(expr);
                }
            }
        }

        if !kwargs_keys.is_empty() {
            args.push(ast::Expr::Map(ast::Spanned::new(
                ast::Map {
                    keys: kwargs_keys,
                    values: kwargs_values,
                },
                self.stream.expand_span(first_span.unwrap()),
            )));
        }

        expect_token!(self, Token::ParenClose, "`)`")?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<ast::Expr<'a>, Error> {
        let (token, span) = expect_token!(self, "expression")?;
        macro_rules! const_val {
            ($expr:expr) => {
                make_const(Value::from($expr), span)
            };
        }

        match token {
            Token::Ident("true" | "True") => Ok(const_val!(true)),
            Token::Ident("false" | "False") => Ok(const_val!(false)),
            Token::Ident("none" | "None") => Ok(const_val!(())),
            Token::Ident(name) => Ok(ast::Expr::Var(Spanned::new(ast::Var { id: name }, span))),
            Token::Str(val) => Ok(const_val!(val)),
            Token::Int(val) => Ok(const_val!(val)),
            Token::Float(val) => Ok(const_val!(val)),
            Token::ParenOpen => self.parse_tuple_or_expression(span),
            Token::BracketOpen => self.parse_list_expr(span),
            Token::BraceOpen => self.parse_map_expr(span),
            token => syntax_error!("unexpected {}", token),
        }
    }

    fn parse_list_expr(&mut self, span: Span) -> Result<ast::Expr<'a>, Error> {
        let mut items = Vec::new();
        loop {
            if matches!(self.stream.current()?, Some((Token::BracketClose, _))) {
                break;
            }
            if !items.is_empty() {
                expect_token!(self, Token::Comma, "`,`")?;
            }
            items.push(self.parse_expr()?);
        }
        expect_token!(self, Token::BracketClose, "`]`")?;
        Ok(ast::Expr::List(Spanned::new(
            ast::List { items },
            self.stream.expand_span(span),
        )))
    }

    fn parse_map_expr(&mut self, span: Span) -> Result<ast::Expr<'a>, Error> {
        let mut keys = Vec::new();
        let mut values = Vec::new();
        loop {
            if matches!(self.stream.current()?, Some((Token::BraceClose, _))) {
                break;
            }
            if !keys.is_empty() {
                expect_token!(self, Token::Comma, "`,`")?;
            }
            keys.push(self.parse_expr()?);
            expect_token!(self, Token::Colon, "`:`")?;
            values.push(self.parse_expr()?);
        }
        expect_token!(self, Token::BraceClose, "`]`")?;
        Ok(ast::Expr::Map(Spanned::new(
            ast::Map { keys, values },
            self.stream.expand_span(span),
        )))
    }

    fn parse_tuple_or_expression(&mut self, span: Span) -> Result<ast::Expr<'a>, Error> {
        // MiniJinja does not really have tuples, but it treats the tuple
        // syntax the same as lists.
        if matches!(self.stream.current()?, Some((Token::ParenClose, _))) {
            self.stream.next()?;
            return Ok(ast::Expr::List(Spanned::new(
                ast::List { items: vec![] },
                self.stream.expand_span(span),
            )));
        }
        let mut expr = self.parse_expr()?;
        if matches!(self.stream.current()?, Some((Token::Comma, _))) {
            let mut items = vec![expr];
            loop {
                if matches!(self.stream.current()?, Some((Token::ParenClose, _))) {
                    break;
                }
                expect_token!(self, Token::Comma, "`,`")?;
                if matches!(self.stream.current()?, Some((Token::ParenClose, _))) {
                    break;
                }
                items.push(self.parse_expr()?);
            }
            expr = ast::Expr::List(Spanned::new(
                ast::List { items },
                self.stream.expand_span(span),
            ));
        }
        expect_token!(self, Token::ParenClose, "`)`")?;
        Ok(expr)
    }

    pub fn parse_expr(&mut self) -> Result<ast::Expr<'a>, Error> {
        self.parse_ifexpr()
    }

    pub fn parse_expr_noif(&mut self) -> Result<ast::Expr<'a>, Error> {
        self.parse_or()
    }

    fn parse_stmt(&mut self) -> Result<ast::Stmt<'a>, Error> {
        let (token, span) = expect_token!(self, "block keyword")?;
        match token {
            Token::Ident("for") => Ok(ast::Stmt::ForLoop(Spanned::new(
                self.parse_for_stmt()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident("if") => Ok(ast::Stmt::IfCond(Spanned::new(
                self.parse_if_cond()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident("with") => Ok(ast::Stmt::WithBlock(Spanned::new(
                self.parse_with_block()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident("set") => Ok(match self.parse_set()? {
                SetParseResult::Set(rv) => {
                    ast::Stmt::Set(Spanned::new(rv, self.stream.expand_span(span)))
                }
                SetParseResult::SetBlock(rv) => {
                    ast::Stmt::SetBlock(Spanned::new(rv, self.stream.expand_span(span)))
                }
            }),
            Token::Ident("block") => Ok(ast::Stmt::Block(Spanned::new(
                self.parse_block()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident("extends") => Ok(ast::Stmt::Extends(Spanned::new(
                self.parse_extends()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident("include") => Ok(ast::Stmt::Include(Spanned::new(
                self.parse_include()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident("autoescape") => Ok(ast::Stmt::AutoEscape(Spanned::new(
                self.parse_auto_escape()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident("filter") => Ok(ast::Stmt::FilterBlock(Spanned::new(
                self.parse_filter_block()?,
                self.stream.expand_span(span),
            ))),
            Token::Ident(name) => syntax_error!("unknown statement {}", name),
            token => syntax_error!("unknown {}, expected statement", token),
        }
    }

    fn parse_assign_name(&mut self) -> Result<ast::Expr<'a>, Error> {
        let (id, span) = expect_token!(self, Token::Ident(name) => name, "identifier")?;
        if RESERVED_NAMES.contains(&id) {
            syntax_error!("cannot assign to reserved variable name {}", id);
        }
        Ok(ast::Expr::Var(ast::Spanned::new(ast::Var { id }, span)))
    }

    fn parse_assignment(&mut self) -> Result<ast::Expr<'a>, Error> {
        let span = self.stream.current_span();
        let mut items = Vec::new();
        let mut is_tuple = false;

        loop {
            if !items.is_empty() {
                expect_token!(self, Token::Comma, "`,`")?;
            }
            if matches!(
                self.stream.current()?,
                Some((
                    Token::ParenClose
                        | Token::VariableEnd(..)
                        | Token::BlockEnd(..)
                        | Token::Ident("in"),
                    _
                ))
            ) {
                break;
            }
            items.push(
                if matches!(self.stream.current()?, Some((Token::ParenOpen, _))) {
                    self.stream.next()?;
                    let rv = self.parse_assignment()?;
                    expect_token!(self, Token::ParenClose, "`)`")?;
                    rv
                } else {
                    self.parse_assign_name()?
                },
            );
            if matches!(self.stream.current()?, Some((Token::Comma, _))) {
                is_tuple = true;
            } else {
                break;
            }
        }

        if !is_tuple && items.len() == 1 {
            Ok(items.into_iter().next().unwrap())
        } else {
            Ok(ast::Expr::List(Spanned::new(
                ast::List { items },
                self.stream.expand_span(span),
            )))
        }
    }

    fn parse_for_stmt(&mut self) -> Result<ast::ForLoop<'a>, Error> {
        let target = self.parse_assignment()?;
        expect_token!(self, Token::Ident("in"), "in")?;
        let iter = self.parse_expr_noif()?;
        let filter_expr = if matches!(self.stream.current()?, Some((Token::Ident("if"), _))) {
            self.stream.next()?;
            Some(self.parse_expr()?)
        } else {
            None
        };
        let recursive = if matches!(self.stream.current()?, Some((Token::Ident("recursive"), _))) {
            self.stream.next()?;
            true
        } else {
            false
        };
        expect_token!(self, Token::BlockEnd(..), "end of block")?;
        let body = self.subparse(&|tok| matches!(tok, Token::Ident("endfor" | "else")))?;
        let else_body = if matches!(self.stream.current()?, Some((Token::Ident("else"), _))) {
            self.stream.next()?;
            expect_token!(self, Token::BlockEnd(..), "end of block")?;
            self.subparse(&|tok| matches!(tok, Token::Ident("endfor")))?
        } else {
            Vec::new()
        };
        self.stream.next()?;
        Ok(ast::ForLoop {
            target,
            iter,
            filter_expr,
            recursive,
            body,
            else_body,
        })
    }

    fn parse_if_cond(&mut self) -> Result<ast::IfCond<'a>, Error> {
        let expr = self.parse_expr_noif()?;
        expect_token!(self, Token::BlockEnd(..), "end of block")?;
        let true_body =
            self.subparse(&|tok| matches!(tok, Token::Ident("endif" | "else" | "elif")))?;
        let false_body = match self.stream.next()? {
            Some((Token::Ident("else"), _)) => {
                expect_token!(self, Token::BlockEnd(..), "end of block")?;
                let rv = self.subparse(&|tok| matches!(tok, Token::Ident("endif")))?;
                self.stream.next()?;
                rv
            }
            Some((Token::Ident("elif"), span)) => vec![ast::Stmt::IfCond(Spanned::new(
                self.parse_if_cond()?,
                self.stream.expand_span(span),
            ))],
            _ => Vec::new(),
        };

        Ok(ast::IfCond {
            expr,
            true_body,
            false_body,
        })
    }

    fn parse_with_block(&mut self) -> Result<ast::WithBlock<'a>, Error> {
        let mut assignments = Vec::new();

        while !matches!(self.stream.current()?, Some((Token::BlockEnd(_), _))) {
            if !assignments.is_empty() {
                expect_token!(self, Token::Comma, "comma")?;
            }
            let target = if matches!(self.stream.current()?, Some((Token::ParenOpen, _))) {
                self.stream.next()?;
                let assign = self.parse_assignment()?;
                expect_token!(self, Token::ParenClose, "`)`")?;
                assign
            } else {
                self.parse_assign_name()?
            };
            expect_token!(self, Token::Assign, "assignment operator")?;
            let expr = self.parse_expr()?;
            assignments.push((target, expr));
        }

        expect_token!(self, Token::BlockEnd(..), "end of block")?;
        let body = self.subparse(&|tok| matches!(tok, Token::Ident("endwith")))?;
        self.stream.next()?;
        Ok(ast::WithBlock { assignments, body })
    }

    fn parse_set(&mut self) -> Result<SetParseResult<'a>, Error> {
        let (target, in_paren) = if matches!(self.stream.current()?, Some((Token::ParenOpen, _))) {
            self.stream.next()?;
            let assign = self.parse_assignment()?;
            expect_token!(self, Token::ParenClose, "`)`")?;
            (assign, true)
        } else {
            (self.parse_assign_name()?, false)
        };

        if !in_paren
            && matches!(
                self.stream.current()?,
                Some((Token::BlockEnd(..) | Token::Pipe, _))
            )
        {
            let filter = if matches!(self.stream.current()?, Some((Token::Pipe, _))) {
                self.stream.next()?;
                Some(self.parse_filter_chain()?)
            } else {
                None
            };
            expect_token!(self, Token::BlockEnd(..), "end of block")?;
            let body = self.subparse(&|tok| matches!(tok, Token::Ident("endset")))?;
            self.stream.next()?;
            Ok(SetParseResult::SetBlock(ast::SetBlock {
                target,
                filter,
                body,
            }))
        } else {
            expect_token!(self, Token::Assign, "assignment operator")?;
            let expr = self.parse_expr()?;
            Ok(SetParseResult::Set(ast::Set { target, expr }))
        }
    }

    fn parse_block(&mut self) -> Result<ast::Block<'a>, Error> {
        let (name, _) = expect_token!(self, Token::Ident(name) => name, "identifier")?;
        expect_token!(self, Token::BlockEnd(..), "end of block")?;
        let body = self.subparse(&|tok| matches!(tok, Token::Ident("endblock")))?;
        self.stream.next()?;

        if let Some((Token::Ident(trailing_name), _)) = self.stream.current()? {
            if *trailing_name != name {
                syntax_error!(
                    "mismatching name on block. Got `{}`, expected `{}`",
                    *trailing_name,
                    name
                );
            }
            self.stream.next()?;
        }

        Ok(ast::Block { name, body })
    }

    fn parse_extends(&mut self) -> Result<ast::Extends<'a>, Error> {
        let name = self.parse_expr()?;
        Ok(ast::Extends { name })
    }

    fn parse_include(&mut self) -> Result<ast::Include<'a>, Error> {
        let name = self.parse_expr()?;
        let ignore_missing = if matches!(self.stream.current()?, Some((Token::Ident("ignore"), _)))
        {
            self.stream.next()?;
            expect_token!(self, Token::Ident("missing"), "missing keyword")?;
            true
        } else {
            false
        };
        Ok(ast::Include {
            name,
            ignore_missing,
        })
    }

    fn parse_auto_escape(&mut self) -> Result<ast::AutoEscape<'a>, Error> {
        let enabled = self.parse_expr()?;
        expect_token!(self, Token::BlockEnd(..), "end of block")?;
        let body = self.subparse(&|tok| matches!(tok, Token::Ident("endautoescape")))?;
        self.stream.next()?;
        Ok(ast::AutoEscape { enabled, body })
    }

    fn parse_filter_chain(&mut self) -> Result<ast::Expr<'a>, Error> {
        let mut filter = None;

        while !matches!(self.stream.current()?, Some((Token::BlockEnd(..), _))) {
            if filter.is_some() {
                expect_token!(self, Token::Pipe, "`|`")?;
            }
            let (name, span) = expect_token!(self, Token::Ident(name) => name, "identifier")?;
            let args = if matches!(self.stream.current()?, Some((Token::ParenOpen, _))) {
                self.parse_args()?
            } else {
                Vec::new()
            };
            filter = Some(ast::Expr::Filter(Spanned::new(
                ast::Filter {
                    name,
                    expr: filter,
                    args,
                },
                self.stream.expand_span(span),
            )));
        }

        filter.ok_or_else(|| Error::new(ErrorKind::SyntaxError, "expected a filter"))
    }

    fn parse_filter_block(&mut self) -> Result<ast::FilterBlock<'a>, Error> {
        let filter = self.parse_filter_chain()?;
        expect_token!(self, Token::BlockEnd(..), "end of block")?;
        let body = self.subparse(&|tok| matches!(tok, Token::Ident("endfilter")))?;
        self.stream.next()?;
        Ok(ast::FilterBlock { filter, body })
    }

    fn subparse(
        &mut self,
        end_check: &dyn Fn(&Token) -> bool,
    ) -> Result<Vec<ast::Stmt<'a>>, Error> {
        let mut rv = Vec::new();
        while let Some((token, span)) = self.stream.next()? {
            match token {
                Token::TemplateData(raw) => {
                    rv.push(ast::Stmt::EmitRaw(Spanned::new(ast::EmitRaw { raw }, span)))
                }
                Token::VariableStart(_) => {
                    let expr = self.parse_expr()?;
                    rv.push(ast::Stmt::EmitExpr(Spanned::new(
                        ast::EmitExpr { expr },
                        self.stream.expand_span(span),
                    )));
                    expect_token!(self, Token::VariableEnd(..), "end of variable block")?;
                }
                Token::BlockStart(_) => {
                    let (tok, _span) = match self.stream.current()? {
                        Some(rv) => rv,
                        None => syntax_error!("unexpected end of input, expected keyword"),
                    };
                    if end_check(tok) {
                        return Ok(rv);
                    }
                    rv.push(self.parse_stmt()?);
                    expect_token!(self, Token::BlockEnd(..), "end of block")?;
                }
                _ => unreachable!("lexer produced garbage"),
            }
        }
        Ok(rv)
    }

    pub fn parse(&mut self) -> Result<ast::Stmt<'a>, Error> {
        // start the stream
        self.stream.next()?;
        let span = self.stream.last_span();
        Ok(ast::Stmt::Template(Spanned::new(
            ast::Template {
                children: self.subparse(&|_| false)?,
            },
            self.stream.expand_span(span),
        )))
    }
}

/// Parses a template
pub fn parse<'source, 'name>(
    source: &'source str,
    filename: &'name str,
) -> Result<ast::Stmt<'source>, Error> {
    // we want to chop off a single newline at the end.  This means that a template
    // by default does not end in a newline which is a useful property to allow
    // inline templates to work.  If someone wants a trailing newline the expectation
    // is that the user adds it themselves for achieve consistency.
    let mut source = source;
    if source.ends_with('\n') {
        source = &source[..source.len() - 1];
    }
    if source.ends_with('\r') {
        source = &source[..source.len() - 1];
    }

    let mut parser = Parser::new(source, false);
    parser.parse().map_err(|mut err| {
        if err.line().is_none() {
            err.set_filename_and_span(filename, parser.stream.last_span())
        }
        err
    })
}

/// Parses an expression
pub fn parse_expr(source: &str) -> Result<ast::Expr<'_>, Error> {
    let mut parser = Parser::new(source, true);
    parser.parse_expr().map_err(|mut err| {
        if err.line().is_none() {
            err.set_filename_and_span("<expression>", parser.stream.last_span())
        }
        err
    })
}
