use std::collections::BTreeSet;
use std::fmt;

use crate::compiler::ast::{self, Spanned};
use crate::compiler::lexer::tokenize;
use crate::compiler::tokens::{Span, Token};
use crate::error::{Error, ErrorKind};
use crate::value::Value;

const MAX_RECURSION: usize = 150;
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
        match ok!($parser.stream.next()) {
            Some(rv) => rv,
            None => return Err(unexpected_eof($expectation)),
        }
    }};
    ($parser:expr, $match:pat, $expectation:expr) => {{
        match ok!($parser.stream.next()) {
            Some((token @ $match, span)) => (token, span),
            Some((token, _)) => return Err(unexpected(token, $expectation)),
            None => return Err(unexpected_eof($expectation)),
        }
    }};
    ($parser:expr, $match:pat => $target:expr, $expectation:expr) => {{
        match ok!($parser.stream.next()) {
            Some(($match, span)) => ($target, span),
            Some((token, _)) => return Err(unexpected(token, $expectation)),
            None => return Err(unexpected_eof($expectation)),
        }
    }};
}

macro_rules! matches_token {
    ($p:expr, $match:pat) => {
        match $p.stream.current() {
            Err(err) => return Err(err),
            Ok(Some(($match, _))) => true,
            _ => false,
        }
    };
}

macro_rules! skip_token {
    ($p:expr, $match:pat) => {
        if matches_token!($p, $match) {
            // we can ignore the error as matches_token! would already
            // have created it.
            let _ = $p.stream.next();
            true
        } else {
            false
        }
    };
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
        let mut iter = Box::new(tokenize(source, in_expr)) as Box<dyn Iterator<Item = _>>;
        let current = iter.next();
        TokenStream {
            iter,
            current,
            last_span: Span::default(),
        }
    }

    /// Advance the stream.
    pub fn next(&mut self) -> Result<Option<(Token<'a>, Span)>, Error> {
        let rv = self.current.take();
        self.current = self.iter.next();
        if let Some(Ok((_, span))) = rv {
            self.last_span = span;
        }
        rv.transpose()
    }

    /// Look at the current token
    pub fn current(&mut self) -> Result<Option<(&Token<'a>, Span)>, Error> {
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
    #[allow(unused)]
    in_macro: bool,
    #[allow(unused)]
    blocks: BTreeSet<&'a str>,
    depth: usize,
}

macro_rules! binop {
    ($func:ident, $next:ident, { $($tok:tt)* }) => {
        fn $func(&mut self) -> Result<ast::Expr<'a>, Error> {
            let span = self.stream.current_span();
            let mut left = ok!(self.$next());
            loop {
                let op = match ok!(self.stream.current()) {
                    $($tok)*
                    _ => break,
                };
                ok!(self.stream.next());
                let right = ok!(self.$next());
                left = ast::Expr::BinOp(Spanned::new(
                    ast::BinOp { op, left, right, },
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
            let op = match ok!(self.stream.current()) {
                $($tok)*
                _ => return self.$next()
            };
            ok!(self.stream.next());
            Ok(ast::Expr::UnaryOp(Spanned::new(
                ast::UnaryOp {
                    op,
                    expr: ok!(self.$func()),
                },
                self.stream.expand_span(span),
            )))
        }
    };
}

macro_rules! with_recursion_guard {
    ($parser:expr, $expr:expr) => {{
        $parser.depth += 1;
        if $parser.depth > MAX_RECURSION {
            return Err(Error::new(
                ErrorKind::SyntaxError,
                "template exceeds maximum recusion limits",
            ));
        }
        let rv = $expr;
        $parser.depth -= 1;
        rv
    }};
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, in_expr: bool) -> Parser<'a> {
        Parser {
            stream: TokenStream::new(source, in_expr),
            in_macro: false,
            blocks: BTreeSet::new(),
            depth: 0,
        }
    }

    fn parse_ifexpr(&mut self) -> Result<ast::Expr<'a>, Error> {
        let mut span = self.stream.last_span();
        let mut expr = ok!(self.parse_or());
        loop {
            if skip_token!(self, Token::Ident("if")) {
                let expr2 = ok!(self.parse_or());
                let expr3 = if skip_token!(self, Token::Ident("else")) {
                    Some(ok!(self.parse_ifexpr()))
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
        let mut expr = ok!(self.parse_math1());
        loop {
            let mut negated = false;
            let op = match ok!(self.stream.current()) {
                Some((Token::Eq, _)) => ast::BinOpKind::Eq,
                Some((Token::Ne, _)) => ast::BinOpKind::Ne,
                Some((Token::Lt, _)) => ast::BinOpKind::Lt,
                Some((Token::Lte, _)) => ast::BinOpKind::Lte,
                Some((Token::Gt, _)) => ast::BinOpKind::Gt,
                Some((Token::Gte, _)) => ast::BinOpKind::Gte,
                Some((Token::Ident("in"), _)) => ast::BinOpKind::In,
                Some((Token::Ident("not"), _)) => {
                    ok!(self.stream.next());
                    expect_token!(self, Token::Ident("in"), "in");
                    negated = true;
                    ast::BinOpKind::In
                }
                _ => break,
            };
            if !negated {
                ok!(self.stream.next());
            }
            expr = ast::Expr::BinOp(Spanned::new(
                ast::BinOp {
                    op,
                    left: expr,
                    right: ok!(self.parse_math1()),
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
        let mut expr = ok!(self.parse_unary_only());
        expr = ok!(self.parse_postfix(expr, span));
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
            match ok!(self.stream.current()) {
                Some((Token::Dot, _)) => {
                    ok!(self.stream.next());
                    let (name, _) = expect_token!(self, Token::Ident(name) => name, "identifier");
                    expr = ast::Expr::GetAttr(Spanned::new(
                        ast::GetAttr { name, expr },
                        self.stream.expand_span(span),
                    ));
                }
                Some((Token::BracketOpen, _)) => {
                    ok!(self.stream.next());

                    let mut start = None;
                    let mut stop = None;
                    let mut step = None;
                    let mut is_slice = false;

                    if !matches_token!(self, Token::Colon) {
                        start = Some(ok!(self.parse_expr()));
                    }
                    if skip_token!(self, Token::Colon) {
                        is_slice = true;
                        if !matches_token!(self, Token::BracketClose | Token::Colon) {
                            stop = Some(ok!(self.parse_expr()));
                        }
                        if skip_token!(self, Token::Colon)
                            && !matches_token!(self, Token::BracketClose)
                        {
                            step = Some(ok!(self.parse_expr()));
                        }
                    }
                    expect_token!(self, Token::BracketClose, "`]`");

                    if !is_slice {
                        expr = ast::Expr::GetItem(Spanned::new(
                            ast::GetItem {
                                expr,
                                subscript_expr: ok!(start.ok_or_else(|| {
                                    Error::new(ErrorKind::SyntaxError, "empty subscript")
                                })),
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
                    let args = ok!(self.parse_args());
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
            match ok!(self.stream.current()) {
                Some((Token::Pipe, _)) => {
                    ok!(self.stream.next());
                    let (name, span) =
                        expect_token!(self, Token::Ident(name) => name, "identifier");
                    let args = if matches_token!(self, Token::ParenOpen) {
                        ok!(self.parse_args())
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
                    ok!(self.stream.next());
                    let negated = skip_token!(self, Token::Ident("not"));
                    let (name, span) =
                        expect_token!(self, Token::Ident(name) => name, "identifier");
                    let args = if matches_token!(self, Token::ParenOpen) {
                        ok!(self.parse_args())
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
        let mut kwargs = Vec::new();

        expect_token!(self, Token::ParenOpen, "`(`");
        loop {
            if skip_token!(self, Token::ParenClose) {
                break;
            }
            if !args.is_empty() || !kwargs.is_empty() {
                expect_token!(self, Token::Comma, "`,`");
                if skip_token!(self, Token::ParenClose) {
                    break;
                }
            }
            let expr = ok!(self.parse_expr());

            // keyword argument
            match expr {
                ast::Expr::Var(ref var) if skip_token!(self, Token::Assign) => {
                    if first_span.is_none() {
                        first_span = Some(var.span());
                    }
                    kwargs.push((var.id, ok!(self.parse_expr_noif())));
                }
                _ if !kwargs.is_empty() => {
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

        if !kwargs.is_empty() {
            args.push(ast::Expr::Kwargs(ast::Spanned::new(
                ast::Kwargs { pairs: kwargs },
                self.stream.expand_span(first_span.unwrap()),
            )));
        };

        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<ast::Expr<'a>, Error> {
        with_recursion_guard!(self, self.parse_primary_impl())
    }

    fn parse_primary_impl(&mut self) -> Result<ast::Expr<'a>, Error> {
        let (token, span) = expect_token!(self, "expression");
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
            Token::String(val) => Ok(const_val!(val)),
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
            if skip_token!(self, Token::BracketClose) {
                break;
            }
            if !items.is_empty() {
                expect_token!(self, Token::Comma, "`,`");
                if skip_token!(self, Token::BracketClose) {
                    break;
                }
            }
            items.push(ok!(self.parse_expr()));
        }
        Ok(ast::Expr::List(Spanned::new(
            ast::List { items },
            self.stream.expand_span(span),
        )))
    }

    fn parse_map_expr(&mut self, span: Span) -> Result<ast::Expr<'a>, Error> {
        let mut keys = Vec::new();
        let mut values = Vec::new();
        loop {
            if skip_token!(self, Token::BraceClose) {
                break;
            }
            if !keys.is_empty() {
                expect_token!(self, Token::Comma, "`,`");
                if skip_token!(self, Token::BraceClose) {
                    break;
                }
            }
            keys.push(ok!(self.parse_expr()));
            expect_token!(self, Token::Colon, "`:`");
            values.push(ok!(self.parse_expr()));
        }
        Ok(ast::Expr::Map(Spanned::new(
            ast::Map { keys, values },
            self.stream.expand_span(span),
        )))
    }

    fn parse_tuple_or_expression(&mut self, span: Span) -> Result<ast::Expr<'a>, Error> {
        // MiniJinja does not really have tuples, but it treats the tuple
        // syntax the same as lists.
        if skip_token!(self, Token::ParenClose) {
            return Ok(ast::Expr::List(Spanned::new(
                ast::List { items: vec![] },
                self.stream.expand_span(span),
            )));
        }
        let mut expr = ok!(self.parse_expr());
        if matches_token!(self, Token::Comma) {
            let mut items = vec![expr];
            loop {
                if skip_token!(self, Token::ParenClose) {
                    break;
                }
                expect_token!(self, Token::Comma, "`,`");
                if skip_token!(self, Token::ParenClose) {
                    break;
                }
                items.push(ok!(self.parse_expr()));
            }
            expr = ast::Expr::List(Spanned::new(
                ast::List { items },
                self.stream.expand_span(span),
            ));
        } else {
            expect_token!(self, Token::ParenClose, "`)`");
        }
        Ok(expr)
    }

    pub fn parse_expr(&mut self) -> Result<ast::Expr<'a>, Error> {
        with_recursion_guard!(self, self.parse_ifexpr())
    }

    pub fn parse_expr_noif(&mut self) -> Result<ast::Expr<'a>, Error> {
        self.parse_or()
    }

    fn parse_stmt(&mut self) -> Result<ast::Stmt<'a>, Error> {
        with_recursion_guard!(self, self.parse_stmt_unprotected())
    }

    fn parse_stmt_unprotected(&mut self) -> Result<ast::Stmt<'a>, Error> {
        let (token, span) = expect_token!(self, "block keyword");

        macro_rules! respan {
            ($expr:expr) => {
                Spanned::new($expr, self.stream.expand_span(span))
            };
        }

        Ok(match token {
            Token::Ident("for") => ast::Stmt::ForLoop(respan!(ok!(self.parse_for_stmt()))),
            Token::Ident("if") => ast::Stmt::IfCond(respan!(ok!(self.parse_if_cond()))),
            Token::Ident("with") => ast::Stmt::WithBlock(respan!(ok!(self.parse_with_block()))),
            Token::Ident("set") => match ok!(self.parse_set()) {
                SetParseResult::Set(rv) => ast::Stmt::Set(respan!(rv)),
                SetParseResult::SetBlock(rv) => ast::Stmt::SetBlock(respan!(rv)),
            },
            Token::Ident("autoescape") => {
                ast::Stmt::AutoEscape(respan!(ok!(self.parse_auto_escape())))
            }
            Token::Ident("filter") => {
                ast::Stmt::FilterBlock(respan!(ok!(self.parse_filter_block())))
            }
            #[cfg(feature = "multi-template")]
            Token::Ident("block") => ast::Stmt::Block(respan!(ok!(self.parse_block()))),
            #[cfg(feature = "multi-template")]
            Token::Ident("extends") => ast::Stmt::Extends(respan!(ok!(self.parse_extends()))),
            #[cfg(feature = "multi-template")]
            Token::Ident("include") => ast::Stmt::Include(respan!(ok!(self.parse_include()))),
            #[cfg(feature = "multi-template")]
            Token::Ident("import") => ast::Stmt::Import(respan!(ok!(self.parse_import()))),
            #[cfg(feature = "multi-template")]
            Token::Ident("from") => ast::Stmt::FromImport(respan!(ok!(self.parse_from_import()))),
            #[cfg(feature = "macros")]
            Token::Ident("macro") => ast::Stmt::Macro(respan!(ok!(self.parse_macro()))),
            Token::Ident(name) => syntax_error!("unknown statement {}", name),
            token => syntax_error!("unknown {}, expected statement", token),
        })
    }

    fn parse_assign_name(&mut self) -> Result<ast::Expr<'a>, Error> {
        let (id, span) = expect_token!(self, Token::Ident(name) => name, "identifier");
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
                expect_token!(self, Token::Comma, "`,`");
            }
            if matches_token!(
                self,
                Token::ParenClose
                    | Token::VariableEnd(..)
                    | Token::BlockEnd(..)
                    | Token::Ident("in")
            ) {
                break;
            }
            items.push(if skip_token!(self, Token::ParenOpen) {
                let rv = ok!(self.parse_assignment());
                expect_token!(self, Token::ParenClose, "`)`");
                rv
            } else {
                ok!(self.parse_assign_name())
            });
            if matches_token!(self, Token::Comma) {
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
        let target = ok!(self.parse_assignment());
        expect_token!(self, Token::Ident("in"), "in");
        let iter = ok!(self.parse_expr_noif());
        let filter_expr = if skip_token!(self, Token::Ident("if")) {
            Some(ok!(self.parse_expr()))
        } else {
            None
        };
        let recursive = skip_token!(self, Token::Ident("recursive"));
        expect_token!(self, Token::BlockEnd(..), "end of block");
        let body = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endfor" | "else"))));
        let else_body = if skip_token!(self, Token::Ident("else")) {
            expect_token!(self, Token::BlockEnd(..), "end of block");
            ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endfor"))))
        } else {
            Vec::new()
        };
        ok!(self.stream.next());
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
        let expr = ok!(self.parse_expr_noif());
        expect_token!(self, Token::BlockEnd(..), "end of block");
        let true_body =
            ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endif" | "else" | "elif"))));
        let false_body = match ok!(self.stream.next()) {
            Some((Token::Ident("else"), _)) => {
                expect_token!(self, Token::BlockEnd(..), "end of block");
                let rv = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endif"))));
                ok!(self.stream.next());
                rv
            }
            Some((Token::Ident("elif"), span)) => vec![ast::Stmt::IfCond(Spanned::new(
                ok!(self.parse_if_cond()),
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

        while !matches_token!(self, Token::BlockEnd(_)) {
            if !assignments.is_empty() {
                expect_token!(self, Token::Comma, "comma");
            }
            let target = if skip_token!(self, Token::ParenOpen) {
                let assign = ok!(self.parse_assignment());
                expect_token!(self, Token::ParenClose, "`)`");
                assign
            } else {
                ok!(self.parse_assign_name())
            };
            expect_token!(self, Token::Assign, "assignment operator");
            let expr = ok!(self.parse_expr());
            assignments.push((target, expr));
        }

        expect_token!(self, Token::BlockEnd(..), "end of block");
        let body = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endwith"))));
        ok!(self.stream.next());
        Ok(ast::WithBlock { assignments, body })
    }

    fn parse_set(&mut self) -> Result<SetParseResult<'a>, Error> {
        let (target, in_paren) = if skip_token!(self, Token::ParenOpen) {
            let assign = ok!(self.parse_assignment());
            expect_token!(self, Token::ParenClose, "`)`");
            (assign, true)
        } else {
            (ok!(self.parse_assign_name()), false)
        };

        if !in_paren && matches_token!(self, Token::BlockEnd(..) | Token::Pipe) {
            let filter = if skip_token!(self, Token::Pipe) {
                Some(ok!(self.parse_filter_chain()))
            } else {
                None
            };
            expect_token!(self, Token::BlockEnd(..), "end of block");
            let body = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endset"))));
            ok!(self.stream.next());
            Ok(SetParseResult::SetBlock(ast::SetBlock {
                target,
                filter,
                body,
            }))
        } else {
            expect_token!(self, Token::Assign, "assignment operator");
            let expr = ok!(self.parse_expr());
            Ok(SetParseResult::Set(ast::Set { target, expr }))
        }
    }

    #[cfg(feature = "multi-template")]
    fn parse_block(&mut self) -> Result<ast::Block<'a>, Error> {
        if self.in_macro {
            syntax_error!("block tags in macros are not allowed");
        }
        let (name, _) = expect_token!(self, Token::Ident(name) => name, "identifier");
        if !self.blocks.insert(name) {
            syntax_error!("block '{}' defined twice", name);
        }

        expect_token!(self, Token::BlockEnd(..), "end of block");
        let body = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endblock"))));
        ok!(self.stream.next());

        if let Some((Token::Ident(trailing_name), _)) = ok!(self.stream.current()) {
            if *trailing_name != name {
                syntax_error!(
                    "mismatching name on block. Got `{}`, expected `{}`",
                    *trailing_name,
                    name
                );
            }
            ok!(self.stream.next());
        }

        Ok(ast::Block { name, body })
    }
    fn parse_auto_escape(&mut self) -> Result<ast::AutoEscape<'a>, Error> {
        let enabled = ok!(self.parse_expr());
        expect_token!(self, Token::BlockEnd(..), "end of block");
        let body = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endautoescape"))));
        ok!(self.stream.next());
        Ok(ast::AutoEscape { enabled, body })
    }

    fn parse_filter_chain(&mut self) -> Result<ast::Expr<'a>, Error> {
        let mut filter = None;

        while !matches_token!(self, Token::BlockEnd(..)) {
            if filter.is_some() {
                expect_token!(self, Token::Pipe, "`|`");
            }
            let (name, span) = expect_token!(self, Token::Ident(name) => name, "identifier");
            let args = if matches_token!(self, Token::ParenOpen) {
                ok!(self.parse_args())
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
        let filter = ok!(self.parse_filter_chain());
        expect_token!(self, Token::BlockEnd(..), "end of block");
        let body = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endfilter"))));
        ok!(self.stream.next());
        Ok(ast::FilterBlock { filter, body })
    }

    #[cfg(feature = "multi-template")]
    fn parse_extends(&mut self) -> Result<ast::Extends<'a>, Error> {
        let name = ok!(self.parse_expr());
        Ok(ast::Extends { name })
    }

    #[cfg(feature = "multi-template")]
    fn parse_include(&mut self) -> Result<ast::Include<'a>, Error> {
        let name = ok!(self.parse_expr());
        let ignore_missing = if skip_token!(self, Token::Ident("ignore")) {
            expect_token!(self, Token::Ident("missing"), "missing keyword");
            true
        } else {
            false
        };
        Ok(ast::Include {
            name,
            ignore_missing,
        })
    }

    #[cfg(feature = "multi-template")]
    fn parse_import(&mut self) -> Result<ast::Import<'a>, Error> {
        let expr = ok!(self.parse_expr());
        expect_token!(self, Token::Ident("as"), "as");
        let name = ok!(self.parse_expr());
        Ok(ast::Import { expr, name })
    }

    #[cfg(feature = "multi-template")]
    fn parse_from_import(&mut self) -> Result<ast::FromImport<'a>, Error> {
        let expr = ok!(self.parse_expr());
        let mut names = Vec::new();
        expect_token!(self, Token::Ident("import"), "import");
        loop {
            if matches_token!(self, Token::BlockEnd(_)) {
                break;
            }
            if !names.is_empty() {
                expect_token!(self, Token::Comma, "`,`");
            }
            if matches_token!(self, Token::BlockEnd(_)) {
                break;
            }
            let name = ok!(self.parse_assign_name());
            let alias = if skip_token!(self, Token::Ident("as")) {
                Some(ok!(self.parse_assign_name()))
            } else {
                None
            };
            names.push((name, alias));
        }
        Ok(ast::FromImport { expr, names })
    }

    #[cfg(feature = "macros")]
    fn parse_macro(&mut self) -> Result<ast::Macro<'a>, Error> {
        let (name, _) = expect_token!(self, Token::Ident(name) => name, "identifier");
        expect_token!(self, Token::ParenOpen, "`(`");
        let mut args = Vec::new();
        let mut defaults = Vec::new();
        loop {
            if skip_token!(self, Token::ParenClose) {
                break;
            }
            if !args.is_empty() {
                expect_token!(self, Token::Comma, "`,`");
                if skip_token!(self, Token::ParenClose) {
                    break;
                }
            }
            args.push(ok!(self.parse_assign_name()));
            if skip_token!(self, Token::Assign) {
                defaults.push(ok!(self.parse_expr()));
            } else if !defaults.is_empty() {
                expect_token!(self, Token::Assign, "`=`");
            }
        }
        expect_token!(self, Token::BlockEnd(..), "end of block");
        let old_in_macro = std::mem::replace(&mut self.in_macro, true);
        let body = ok!(self.subparse(&|tok| matches!(tok, Token::Ident("endmacro"))));
        self.in_macro = old_in_macro;
        ok!(self.stream.next());
        Ok(ast::Macro {
            name,
            args,
            defaults,
            body,
        })
    }

    fn subparse(
        &mut self,
        end_check: &dyn Fn(&Token) -> bool,
    ) -> Result<Vec<ast::Stmt<'a>>, Error> {
        let mut rv = Vec::new();
        while let Some((token, span)) = ok!(self.stream.next()) {
            match token {
                Token::TemplateData(raw) => {
                    rv.push(ast::Stmt::EmitRaw(Spanned::new(ast::EmitRaw { raw }, span)))
                }
                Token::VariableStart(_) => {
                    let expr = ok!(self.parse_expr());
                    rv.push(ast::Stmt::EmitExpr(Spanned::new(
                        ast::EmitExpr { expr },
                        self.stream.expand_span(span),
                    )));
                    expect_token!(self, Token::VariableEnd(..), "end of variable block");
                }
                Token::BlockStart(_) => {
                    let (tok, _span) = match ok!(self.stream.current()) {
                        Some(rv) => rv,
                        None => syntax_error!("unexpected end of input, expected keyword"),
                    };
                    if end_check(tok) {
                        return Ok(rv);
                    }
                    rv.push(ok!(self.parse_stmt()));
                    expect_token!(self, Token::BlockEnd(..), "end of block");
                }
                _ => unreachable!("lexer produced garbage"),
            }
        }
        Ok(rv)
    }

    pub fn parse(&mut self) -> Result<ast::Stmt<'a>, Error> {
        let span = self.stream.last_span();
        Ok(ast::Stmt::Template(Spanned::new(
            ast::Template {
                children: ok!(self.subparse(&|_| false)),
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
