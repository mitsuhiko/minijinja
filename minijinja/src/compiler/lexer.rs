use std::borrow::Cow;
use std::ops::ControlFlow;

use crate::compiler::tokens::{Span, Token};
use crate::error::{Error, ErrorKind};
use crate::utils::{memchr, memstr, unescape};

#[cfg(feature = "custom_syntax")]
pub use crate::custom_syntax::SyntaxConfig;

/// Non configurable syntax config
#[cfg(not(feature = "custom_syntax"))]
#[derive(Debug, Clone, Default)]
pub struct SyntaxConfig;

/// Internal config struct to control whitespace in the engine.
#[derive(Copy, Clone, Debug, Default)]
pub struct WhitespaceConfig {
    pub keep_trailing_newline: bool,
    pub lstrip_blocks: bool,
    pub trim_blocks: bool,
}

enum LexerState {
    Template,
    InVariable,
    InBlock,
}

/// Utility enum that defines a marker.
#[derive(Debug, Copy, Clone)]
pub enum StartMarker {
    Variable,
    Block,
    Comment,
}

/// Tokenizes jinja templates.
pub struct Tokenizer<'s> {
    stack: Vec<LexerState>,
    rest: &'s str,
    failed: bool,
    current_line: u32,
    current_col: u32,
    current_offset: u32,
    trim_leading_whitespace: bool,
    syntax_config: SyntaxConfig,
    whitespace_config: WhitespaceConfig,
}

fn find_start_marker_memchr(a: &str) -> Option<(usize, bool)> {
    let bytes = a.as_bytes();
    let mut offset = 0;
    loop {
        let idx = match memchr(&bytes[offset..], b'{') {
            Some(idx) => idx,
            None => return None,
        };
        if let Some(b'{' | b'%' | b'#') = bytes.get(offset + idx + 1).copied() {
            return Some((
                offset + idx,
                bytes.get(offset + idx + 2).copied() == Some(b'-'),
            ));
        }
        offset += idx + 1;
    }
}

#[cfg(feature = "custom_syntax")]
fn find_start_marker(a: &str, syntax_config: &SyntaxConfig) -> Option<(usize, bool)> {
    // If we have a custom delimiter we need to use the aho-corasick
    // otherwise we can use internal memchr.
    match syntax_config.aho_corasick {
        Some(ref ac) => {
            let bytes = a.as_bytes();
            ac.find(bytes).map(|m| {
                (
                    m.start(),
                    bytes.get(m.start() + m.len()).copied() == Some(b'-'),
                )
            })
        }
        None => find_start_marker_memchr(a),
    }
}

#[cfg(not(feature = "custom_syntax"))]
fn find_start_marker(a: &str, _syntax_config: &SyntaxConfig) -> Option<(usize, bool)> {
    find_start_marker_memchr(a)
}

fn match_start_marker(rest: &str, syntax_config: &SyntaxConfig) -> Option<(StartMarker, usize)> {
    #[cfg(not(feature = "custom_syntax"))]
    {
        let _ = syntax_config;
        match_start_marker_default(rest)
    }

    #[cfg(feature = "custom_syntax")]
    {
        if syntax_config.aho_corasick.is_none() {
            return match_start_marker_default(rest);
        }

        for delimiter in syntax_config.start_delimiters_order {
            let marker = match delimiter {
                StartMarker::Variable => &syntax_config.syntax.variable_start as &str,
                StartMarker::Block => &syntax_config.syntax.block_start as &str,
                StartMarker::Comment => &syntax_config.syntax.comment_start as &str,
            };
            if rest.get(..marker.len()) == Some(marker) {
                return Some((delimiter, marker.len()));
            }
        }

        None
    }
}

fn match_start_marker_default(rest: &str) -> Option<(StartMarker, usize)> {
    match rest.get(..2) {
        Some("{{") => Some((StartMarker::Variable, 2)),
        Some("{%") => Some((StartMarker::Block, 2)),
        Some("{#") => Some((StartMarker::Comment, 2)),
        _ => None,
    }
}

macro_rules! syntax_token_getter {
    ($ident:ident, $default:expr) => {
        #[inline]
        fn $ident(&self) -> &str {
            #[cfg(feature = "custom_syntax")]
            {
                &self.syntax_config.syntax.$ident
            }
            #[cfg(not(feature = "custom_syntax"))]
            {
                $default
            }
        }
    };
}

#[cfg(feature = "unicode")]
fn lex_identifier(s: &str) -> usize {
    s.chars()
        .enumerate()
        .map_while(|(idx, c)| {
            let cont = if c == '_' {
                true
            } else if idx == 0 {
                unicode_ident::is_xid_start(c)
            } else {
                unicode_ident::is_xid_continue(c)
            };
            cont.then(|| c.len_utf8())
        })
        .sum::<usize>()
}

#[cfg(not(feature = "unicode"))]
fn lex_identifier(s: &str) -> usize {
    s.as_bytes()
        .iter()
        .enumerate()
        .take_while(|&(idx, &c)| {
            if c == b'_' {
                true
            } else if idx == 0 {
                c.is_ascii_alphabetic()
            } else {
                c.is_ascii_alphanumeric()
            }
        })
        .count()
}

fn skip_basic_tag(block_str: &str, name: &str, block_end: &str) -> Option<(usize, bool)> {
    let mut ptr = block_str;
    let mut trim = false;

    if let Some(rest) = ptr.strip_prefix('-') {
        ptr = rest;
    }
    while let Some(rest) = ptr.strip_prefix(|x: char| x.is_ascii_whitespace()) {
        ptr = rest;
    }

    ptr = match ptr.strip_prefix(name) {
        Some(ptr) => ptr,
        None => return None,
    };

    while let Some(rest) = ptr.strip_prefix(|x: char| x.is_ascii_whitespace()) {
        ptr = rest;
    }
    if let Some(rest) = ptr.strip_prefix('-') {
        ptr = rest;
        trim = true;
    }
    ptr = match ptr.strip_prefix(block_end) {
        Some(ptr) => ptr,
        None => return None,
    };

    Some((block_str.len() - ptr.len(), trim))
}

impl<'s> Tokenizer<'s> {
    /// Creates a new tokenizer.
    pub fn new(
        input: &'s str,
        in_expr: bool,
        syntax_config: SyntaxConfig,
        whitespace_config: WhitespaceConfig,
    ) -> Tokenizer<'s> {
        Tokenizer {
            rest: input,
            stack: vec![if in_expr {
                LexerState::InVariable
            } else {
                LexerState::Template
            }],
            failed: false,
            current_line: 1,
            current_col: 0,
            current_offset: 0,
            trim_leading_whitespace: false,
            syntax_config,
            whitespace_config,
        }
    }

    /// Produces the next token from the tokenizer.
    pub fn next_token(&mut self) -> Result<Option<(Token<'s>, Span)>, Error> {
        loop {
            if self.rest.is_empty() || self.failed {
                return Ok(None);
            }
            let outcome = match self.stack.last() {
                Some(LexerState::Template) => self.tokenize_root(),
                Some(LexerState::InBlock) => self.tokenize_block_or_var(true),
                Some(LexerState::InVariable) => self.tokenize_block_or_var(false),
                None => panic!("empty lexer stack"),
            };
            match ok!(outcome) {
                ControlFlow::Break(rv) => return Ok(Some(rv)),
                ControlFlow::Continue(()) => continue,
            }
        }
    }

    fn advance(&mut self, bytes: usize) -> &'s str {
        let (skipped, new_rest) = self.rest.split_at(bytes);
        for c in skipped.chars() {
            match c {
                '\n' => {
                    self.current_line += 1;
                    self.current_col = 0;
                }
                _ => self.current_col += 1,
            }
        }
        self.current_offset += bytes as u32;
        self.rest = new_rest;
        skipped
    }

    #[inline(always)]
    fn loc(&self) -> (u32, u32, u32) {
        (self.current_line, self.current_col, self.current_offset)
    }

    fn span(&self, (start_line, start_col, start_offset): (u32, u32, u32)) -> Span {
        Span {
            start_line,
            start_col,
            start_offset,
            end_line: self.current_line,
            end_col: self.current_col,
            end_offset: self.current_offset,
        }
    }

    fn syntax_error(&mut self, msg: &'static str) -> Error {
        self.failed = true;
        Error::new(ErrorKind::SyntaxError, msg)
    }

    fn eat_number(&mut self) -> Result<(Token<'s>, Span), Error> {
        #[derive(Copy, Clone)]
        enum State {
            RadixInteger, // 0x10
            Integer,      // 123
            Fraction,     // .123
            Exponent,     // E | e
            ExponentSign, // +|-
        }

        let old_loc = self.loc();

        let radix = match self.rest.as_bytes().get(..2) {
            Some(b"0b" | b"0B") => 2,
            Some(b"0o" | b"0O") => 8,
            Some(b"0x" | b"0X") => 16,
            _ => 10,
        };

        let mut state = if radix == 10 {
            State::Integer
        } else {
            self.advance(2);
            State::RadixInteger
        };

        let mut num_len = self
            .rest
            .as_bytes()
            .iter()
            .take_while(|&c| c.is_ascii_digit())
            .count();
        let mut has_underscore = false;
        for c in self.rest.as_bytes()[num_len..].iter().copied() {
            state = match (c, state) {
                (b'.', State::Integer) => State::Fraction,
                (b'E' | b'e', State::Integer | State::Fraction) => State::Exponent,
                (b'+' | b'-', State::Exponent) => State::ExponentSign,
                (b'0'..=b'9', State::Exponent) => State::ExponentSign,
                (b'0'..=b'9', state) => state,
                (b'a'..=b'f' | b'A'..=b'F', State::RadixInteger) if radix == 16 => state,
                (b'_', _) => {
                    has_underscore = true;
                    state
                }
                _ => break,
            };
            num_len += 1;
        }
        let is_float = !matches!(state, State::Integer | State::RadixInteger);

        let mut num = Cow::Borrowed(self.advance(num_len));
        if has_underscore {
            if num.ends_with('_') {
                return Err(self.syntax_error("'_' may not occur at end of number"));
            }
            num = Cow::Owned(num.replace('_', ""));
        }

        Ok((
            ok!(if is_float {
                num.parse()
                    .map(Token::Float)
                    .map_err(|_| self.syntax_error("invalid float"))
            } else if let Ok(int) = u64::from_str_radix(&num, radix) {
                Ok(Token::Int(int))
            } else {
                u128::from_str_radix(&num, radix)
                    .map(Token::Int128)
                    .map_err(|_| self.syntax_error("invalid integer"))
            }),
            self.span(old_loc),
        ))
    }

    fn eat_identifier(&mut self) -> Result<(Token<'s>, Span), Error> {
        let ident_len = lex_identifier(self.rest);
        if ident_len > 0 {
            let old_loc = self.loc();
            let ident = self.advance(ident_len);
            Ok((Token::Ident(ident), self.span(old_loc)))
        } else {
            Err(self.syntax_error("unexpected character"))
        }
    }

    fn eat_string(&mut self, delim: u8) -> Result<(Token<'s>, Span), Error> {
        let old_loc = self.loc();
        let mut escaped = false;
        let mut has_escapes = false;
        let str_len = self
            .rest
            .as_bytes()
            .iter()
            .skip(1)
            .take_while(|&&c| match (escaped, c) {
                (true, _) => {
                    escaped = false;
                    true
                }
                (_, b'\\') => {
                    escaped = true;
                    has_escapes = true;
                    true
                }
                (_, c) if c == delim => false,
                _ => true,
            })
            .count();
        if escaped || self.rest.as_bytes().get(str_len + 1) != Some(&delim) {
            return Err(self.syntax_error("unexpected end of string"));
        }
        let s = self.advance(str_len + 2);
        Ok(if has_escapes {
            (
                Token::String(match unescape(&s[1..s.len() - 1]) {
                    Ok(unescaped) => unescaped,
                    Err(err) => return Err(err),
                }),
                self.span(old_loc),
            )
        } else {
            (Token::Str(&s[1..s.len() - 1]), self.span(old_loc))
        })
    }

    fn skip_whitespace(&mut self) {
        let skipped = self
            .rest
            .chars()
            .map_while(|c| c.is_whitespace().then(|| c.len_utf8()))
            .sum();
        if skipped > 0 {
            self.advance(skipped);
        }
    }

    fn skip_newline_if_trim_blocks(&mut self) {
        if self.whitespace_config.trim_blocks {
            if self.rest.as_bytes().get(0) == Some(&b'\r') {
                self.advance(1);
            }
            if self.rest.as_bytes().get(0) == Some(&b'\n') {
                self.advance(1);
            }
        }
    }

    syntax_token_getter!(variable_end, "}}");
    syntax_token_getter!(block_start, "{%");
    syntax_token_getter!(block_end, "%}");
    syntax_token_getter!(comment_end, "#}");

    fn tokenize_root(&mut self) -> Result<ControlFlow<(Token<'s>, Span)>, Error> {
        let mut old_loc = self.loc();
        match match_start_marker(self.rest, &self.syntax_config) {
            Some((StartMarker::Comment, skip)) => {
                if let Some(end) =
                    memstr(&self.rest.as_bytes()[skip..], self.comment_end().as_bytes())
                {
                    if self
                        .rest
                        .as_bytes()
                        .get(end.saturating_sub(1) + skip)
                        .copied()
                        == Some(b'-')
                    {
                        self.trim_leading_whitespace = true;
                    }
                    self.advance(end + skip + self.comment_end().len());
                    return Ok(ControlFlow::Continue(()));
                } else {
                    return Err(self.syntax_error("unexpected end of comment"));
                }
            }
            Some((StartMarker::Variable, skip)) => {
                if self.rest.as_bytes().get(skip) == Some(&b'-') {
                    self.advance(skip + 1);
                } else {
                    self.advance(skip);
                }
                self.stack.push(LexerState::InVariable);
                return Ok(ControlFlow::Break((
                    Token::VariableStart,
                    self.span(old_loc),
                )));
            }
            Some((StartMarker::Block, skip)) => {
                // raw blocks require some special handling.  If we are at the beginning of a raw
                // block we want to skip everything until {% endraw %} completely ignoring interior
                // syntax and emit the entire raw block as TemplateData.
                if let Some((raw, trim_start)) =
                    skip_basic_tag(&self.rest[skip..], "raw", self.block_end())
                {
                    self.advance(raw + skip);
                    self.skip_newline_if_trim_blocks();
                    let mut ptr = 0;
                    while let Some(block) =
                        memstr(&self.rest.as_bytes()[ptr..], self.block_start().as_bytes())
                    {
                        ptr += block + self.block_start().len();
                        let trim_end = self.rest.as_bytes().get(ptr) == Some(&b'-');
                        if let Some((endraw, trim_next)) =
                            skip_basic_tag(&self.rest[ptr..], "endraw", self.block_end())
                        {
                            let mut result = &self.rest[..ptr - self.block_start().len()];
                            if trim_start {
                                result = result.trim_start();
                            }
                            if trim_end {
                                result = result.trim_end();
                            }
                            self.advance(ptr + endraw);
                            self.skip_newline_if_trim_blocks();
                            self.trim_leading_whitespace = trim_next;
                            return Ok(ControlFlow::Break((
                                Token::TemplateData(result),
                                self.span(old_loc),
                            )));
                        }
                    }
                    return Err(self.syntax_error("unexpected end of raw block"));
                }

                if self.rest.as_bytes().get(skip) == Some(&b'-') {
                    self.advance(skip + 1);
                } else {
                    self.advance(skip);
                }

                self.stack.push(LexerState::InBlock);
                return Ok(ControlFlow::Break((Token::BlockStart, self.span(old_loc))));
            }
            None => {}
        }
        if self.trim_leading_whitespace {
            self.trim_leading_whitespace = false;
            self.skip_whitespace();
        }
        old_loc = self.loc();
        let (lead, span) = match find_start_marker(self.rest, &self.syntax_config) {
            Some((start, false)) => (self.advance(start), self.span(old_loc)),
            Some((start, _)) => {
                let peeked = &self.rest[..start];
                let trimmed = peeked.trim_end();
                let lead = self.advance(trimmed.len());
                let span = self.span(old_loc);
                self.advance(peeked.len() - trimmed.len());
                (lead, span)
            }
            None => (self.advance(self.rest.len()), self.span(old_loc)),
        };
        if lead.is_empty() {
            Ok(ControlFlow::Continue(()))
        } else {
            Ok(ControlFlow::Break((Token::TemplateData(lead), span)))
        }
    }

    fn tokenize_block_or_var(
        &mut self,
        is_block: bool,
    ) -> Result<ControlFlow<(Token<'s>, Span)>, Error> {
        let old_loc = self.loc();
        // in blocks whitespace is generally ignored, skip it.
        match self
            .rest
            .as_bytes()
            .iter()
            .position(|&x| !x.is_ascii_whitespace())
        {
            Some(0) => {}
            None => {
                self.advance(self.rest.len());
                return Ok(ControlFlow::Continue(()));
            }
            Some(offset) => {
                self.advance(offset);
                return Ok(ControlFlow::Continue(()));
            }
        }

        // look out for the end of blocks
        if is_block {
            if self.rest.get(..1) == Some("-") && self.rest[1..].starts_with(self.block_end()) {
                self.stack.pop();
                self.trim_leading_whitespace = true;
                self.advance(self.block_end().len() + 1);
                let span = self.span(old_loc);
                self.skip_newline_if_trim_blocks();
                return Ok(ControlFlow::Break((Token::BlockEnd, span)));
            }
            if self.rest.starts_with(self.block_end()) {
                self.stack.pop();
                self.advance(self.block_end().len());
                let span = self.span(old_loc);
                self.skip_newline_if_trim_blocks();
                return Ok(ControlFlow::Break((Token::BlockEnd, span)));
            }
        } else {
            if self.rest.get(..1) == Some("-") && self.rest[1..].starts_with(self.variable_end()) {
                self.stack.pop();
                self.advance(self.variable_end().len() + 1);
                self.trim_leading_whitespace = true;
                return Ok(ControlFlow::Break((Token::VariableEnd, self.span(old_loc))));
            }
            if self.rest.starts_with(self.variable_end()) {
                self.stack.pop();
                self.advance(self.variable_end().len());
                return Ok(ControlFlow::Break((Token::VariableEnd, self.span(old_loc))));
            }
        }

        // two character operators
        let op = match self.rest.as_bytes().get(..2) {
            Some(b"//") => Some(Token::FloorDiv),
            Some(b"**") => Some(Token::Pow),
            Some(b"==") => Some(Token::Eq),
            Some(b"!=") => Some(Token::Ne),
            Some(b">=") => Some(Token::Gte),
            Some(b"<=") => Some(Token::Lte),
            _ => None,
        };
        if let Some(op) = op {
            self.advance(2);
            return Ok(ControlFlow::Break((op, self.span(old_loc))));
        }

        // single character operators (and strings)
        let op = match self.rest.as_bytes().get(0) {
            Some(b'+') => Some(Token::Plus),
            Some(b'-') => Some(Token::Minus),
            Some(b'*') => Some(Token::Mul),
            Some(b'/') => Some(Token::Div),
            Some(b'%') => Some(Token::Mod),
            Some(b'!') => Some(Token::Bang),
            Some(b'.') => Some(Token::Dot),
            Some(b',') => Some(Token::Comma),
            Some(b':') => Some(Token::Colon),
            Some(b'~') => Some(Token::Tilde),
            Some(b'|') => Some(Token::Pipe),
            Some(b'=') => Some(Token::Assign),
            Some(b'>') => Some(Token::Gt),
            Some(b'<') => Some(Token::Lt),
            Some(b'(') => Some(Token::ParenOpen),
            Some(b')') => Some(Token::ParenClose),
            Some(b'[') => Some(Token::BracketOpen),
            Some(b']') => Some(Token::BracketClose),
            Some(b'{') => Some(Token::BraceOpen),
            Some(b'}') => Some(Token::BraceClose),
            Some(b'\'') => {
                return Ok(ControlFlow::Break(ok!(self.eat_string(b'\''))));
            }
            Some(b'"') => {
                return Ok(ControlFlow::Break(ok!(self.eat_string(b'"'))));
            }
            Some(c) if c.is_ascii_digit() => return Ok(ControlFlow::Break(ok!(self.eat_number()))),
            _ => None,
        };
        if let Some(op) = op {
            self.advance(1);
            Ok(ControlFlow::Break((op, self.span(old_loc))))
        } else {
            Ok(ControlFlow::Break(ok!(self.eat_identifier())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use similar_asserts::assert_eq;

    pub fn tokenize(
        input: &str,
        in_expr: bool,
        syntax_config: SyntaxConfig,
    ) -> impl Iterator<Item = Result<(Token<'_>, Span), Error>> {
        let mut tokenizer =
            Tokenizer::new(input, in_expr, syntax_config, WhitespaceConfig::default());
        std::iter::from_fn(move || tokenizer.next_token().transpose())
    }

    #[test]
    fn test_find_marker() {
        let syntax = SyntaxConfig::default();
        assert!(find_start_marker("{", &syntax).is_none());
        assert!(find_start_marker("foo", &syntax).is_none());
        assert!(find_start_marker("foo {", &syntax).is_none());
        assert_eq!(find_start_marker("foo {{", &syntax), Some((4, false)));
        assert_eq!(find_start_marker("foo {{-", &syntax), Some((4, true)));
    }

    #[test]
    #[cfg(feature = "custom_syntax")]
    fn test_find_marker_custom_syntax() {
        use crate::Syntax;

        let syntax = Syntax {
            block_start: "%{".into(),
            block_end: "}%".into(),
            variable_start: "[[".into(),
            variable_end: "]]".into(),
            comment_start: "/*".into(),
            comment_end: "*/".into(),
        };

        let syntax_config = syntax.compile().expect("failed to create syntax config");

        assert_eq!(find_start_marker("%{", &syntax_config), Some((0, false)));
        assert!(find_start_marker("/", &syntax_config).is_none());
        assert!(find_start_marker("foo [", &syntax_config).is_none());
        assert_eq!(
            find_start_marker("foo /*", &syntax_config),
            Some((4, false))
        );
        assert_eq!(
            find_start_marker("foo [[-", &syntax_config),
            Some((4, true))
        );
    }

    #[test]
    fn test_is_basic_tag() {
        assert_eq!(skip_basic_tag(" raw %}", "raw", "%}"), Some((7, false)));
        assert_eq!(skip_basic_tag(" raw %}", "endraw", "%}"), None);
        assert_eq!(skip_basic_tag("  raw  %}", "raw", "%}"), Some((9, false)));
        assert_eq!(skip_basic_tag("-  raw  -%}", "raw", "%}"), Some((11, true)));
    }

    #[test]
    fn test_basic_identifiers() {
        fn assert_ident(s: &str) {
            match tokenize(s, true, Default::default()).next() {
                Some(Ok((Token::Ident(ident), _))) if ident == s => {}
                _ => panic!("did not get a matching token result: {s:?}"),
            }
        }

        fn assert_not_ident(s: &str) {
            let res = tokenize(s, true, Default::default()).collect::<Result<Vec<_>, _>>();
            if let Ok(tokens) = res {
                if let &[(Token::Ident(_), _)] = &tokens[..] {
                    panic!("got a single ident for {s:?}")
                }
            }
        }

        assert_ident("foo_bar_baz");
        assert_ident("_foo_bar_baz");
        assert_ident("_42world");
        assert_ident("_world42");
        assert_ident("world42");
        assert_not_ident("42world");

        #[cfg(feature = "unicode")]
        {
            assert_ident("foo");
            assert_ident("föö");
            assert_ident("き");
            assert_ident("_");
            assert_not_ident("1a");
            assert_not_ident("a-");
            assert_not_ident("🐍a");
            assert_not_ident("a🐍🐍");
            assert_ident("ᢅ");
            assert_ident("ᢆ");
            assert_ident("℘");
            assert_ident("℮");
            assert_not_ident("·");
            assert_ident("a·");
        }
    }
}
