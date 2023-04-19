use std::borrow::Borrow;
use std::sync::Arc;

use crate::compiler::tokens::{Span, Token};
use crate::error::{Error, ErrorKind};
use crate::settings::SyntaxConfig;
use crate::utils::{memchr, memstr, unescape};

#[cfg(test)]
use similar_asserts::assert_eq;

enum LexerState {
    Template,
    InVariable,
    InBlock,
}

struct TokenizerState<'s> {
    stack: Vec<LexerState>,
    rest: &'s str,
    failed: bool,
    current_line: u32,
    current_col: u32,
}

fn find_marker_memchr(a: &str) -> Option<(usize, bool)> {
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

#[cfg(feature = "custom_delimiters")]
fn find_marker(a: &str, syntax: &SyntaxConfig) -> Option<(usize, bool)> {
    // If we have a custom delimiter we need to use the aho-corasick
    // otherwise we can use internal memchr.
    match syntax.aho_corasick {
        Some(ref ac) => {
            let bytes = a.as_bytes();
            ac.find(bytes).map(|m| {
                (
                    m.start(),
                    bytes.get(m.start() + m.len()).copied() == Some(b'-'),
                )
            })
        }
        None => find_marker_memchr(a),
    }
}

#[cfg(not(feature = "custom_delimiters"))]
fn find_marker(a: &str, _syntax: &SyntaxConfig) -> Option<(usize, bool)> {
    find_marker_memchr(a)
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

impl<'s> TokenizerState<'s> {
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
        self.rest = new_rest;
        skipped
    }

    #[inline(always)]
    fn loc(&self) -> (u32, u32) {
        (self.current_line, self.current_col)
    }

    fn span(&self, start: (u32, u32)) -> Span {
        let (start_line, start_col) = start;
        Span {
            start_line,
            start_col,
            end_line: self.current_line,
            end_col: self.current_col,
        }
    }

    fn syntax_error(&mut self, msg: &'static str) -> Error {
        self.failed = true;
        Error::new(ErrorKind::SyntaxError, msg)
    }

    fn eat_number(&mut self) -> Result<(Token<'s>, Span), Error> {
        #[derive(Copy, Clone)]
        enum State {
            Integer,      // 123
            Fraction,     // .123
            Exponent,     // E | e
            ExponentSign, // +|-
        }

        let old_loc = self.loc();
        let mut state = State::Integer;
        let mut num_len = self
            .rest
            .chars()
            .take_while(|&c| c.is_ascii_digit())
            .count();
        for c in self.rest.chars().skip(num_len) {
            state = match (c, state) {
                ('.', State::Integer) => State::Fraction,
                ('E' | 'e', State::Integer | State::Fraction) => State::Exponent,
                ('+' | '-', State::Exponent) => State::ExponentSign,
                ('0'..='9', State::Exponent) => State::ExponentSign,
                ('0'..='9', state) => state,
                _ => break,
            };
            num_len += 1;
        }
        let is_float = !matches!(state, State::Integer);

        let num = self.advance(num_len);
        Ok((
            ok!(if is_float {
                num.parse()
                    .map(Token::Float)
                    .map_err(|_| self.syntax_error("invalid float"))
            } else {
                num.parse()
                    .map(Token::Int)
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
        let skip = self
            .rest
            .chars()
            .map_while(|c| c.is_whitespace().then(|| c.len_utf8()))
            .sum::<usize>();
        if skip > 0 {
            self.advance(skip);
        }
    }
}

/// Tokenizes the source.
pub fn tokenize(
    input: &str,
    in_expr: bool,
    syntax_config: Arc<SyntaxConfig>,
) -> impl Iterator<Item = Result<(Token<'_>, Span), Error>> {
    let mut state = TokenizerState {
        rest: input,
        stack: vec![if in_expr {
            LexerState::InVariable
        } else {
            LexerState::Template
        }],
        failed: false,
        current_line: 1,
        current_col: 0,
    };
    let mut trim_leading_whitespace = false;

    std::iter::from_fn(move || loop {
        if state.rest.is_empty() || state.failed {
            return None;
        }

        let mut old_loc = state.loc();
        match state.stack.last() {
            Some(LexerState::Template) => {
                if state.rest.get(..syntax_config.syntax.variable_start.len())
                    == Some(&syntax_config.syntax.variable_start)
                {
                    if state.rest.as_bytes().get(2) == Some(&b'-') {
                        state.advance(syntax_config.syntax.variable_start.len() + 1);
                    } else {
                        state.advance(syntax_config.syntax.variable_start.len());
                    }
                    state.stack.push(LexerState::InVariable);
                    return Some(Ok((Token::VariableStart, state.span(old_loc))));
                }
                if state.rest.get(..syntax_config.syntax.block_start.len())
                    == Some(&syntax_config.syntax.block_start)
                {
                    // raw blocks require some special handling.  If we are at the beginning of a raw
                    // block we want to skip everything until {% endraw %} completely ignoring iterior
                    // syntax and emit the entire raw block as TemplateData.
                    if let Some((mut ptr, _)) = skip_basic_tag(
                        &state.rest[syntax_config.syntax.block_start.len()..],
                        "raw",
                        &syntax_config.syntax.block_end,
                    ) {
                        ptr += syntax_config.syntax.block_start.len();
                        while let Some(block) = memstr(
                            &state.rest.as_bytes()[ptr..],
                            syntax_config.syntax.block_start.as_bytes(),
                        ) {
                            ptr += block + syntax_config.syntax.block_start.len();
                            if let Some((endraw, trim)) = skip_basic_tag(
                                &state.rest[ptr..],
                                "endraw",
                                &syntax_config.syntax.block_end,
                            ) {
                                let result = &state.rest[..ptr + endraw];
                                state.advance(ptr + endraw);
                                trim_leading_whitespace = trim;
                                return Some(Ok((
                                    Token::TemplateData(result),
                                    state.span(old_loc),
                                )));
                            }
                        }
                        return Some(Err(state.syntax_error("unexpected end of raw block")));
                    }

                    if state
                        .rest
                        .as_bytes()
                        .get(syntax_config.syntax.block_start.len())
                        == Some(&b'-')
                    {
                        state.advance(syntax_config.syntax.block_start.len() + 1);
                    } else {
                        state.advance(syntax_config.syntax.block_start.len());
                    }

                    state.stack.push(LexerState::InBlock);
                    return Some(Ok((Token::BlockStart, state.span(old_loc))));
                }
                if state.rest.get(..syntax_config.syntax.comment_start.len())
                    == Some(&syntax_config.syntax.comment_start)
                {
                    if let Some(comment_end) = memstr(
                        state.rest.as_bytes(),
                        syntax_config.syntax.comment_end.as_bytes(),
                    ) {
                        if state
                            .rest
                            .as_bytes()
                            .get(comment_end.saturating_sub(1))
                            .copied()
                            == Some(b'-')
                        {
                            trim_leading_whitespace = true;
                        }
                        state.advance(comment_end + syntax_config.syntax.comment_end.len());
                        continue;
                    } else {
                        return Some(Err(state.syntax_error("unexpected end of comment")));
                    }
                }

                if trim_leading_whitespace {
                    trim_leading_whitespace = false;
                    state.skip_whitespace();
                    old_loc = state.loc();
                }

                let (lead, span) = match find_marker(state.rest, &syntax_config) {
                    Some((start, false)) => (state.advance(start), state.span(old_loc)),
                    Some((start, _)) => {
                        let peeked = &state.rest[..start];
                        let trimmed = peeked.trim_end();
                        let lead = state.advance(trimmed.len());
                        let span = state.span(old_loc);
                        state.advance(peeked.len() - trimmed.len());
                        (lead, span)
                    }
                    None => (state.advance(state.rest.len()), state.span(old_loc)),
                };
                if lead.is_empty() {
                    continue;
                }
                return Some(Ok((Token::TemplateData(lead), span)));
            }
            Some(LexerState::InBlock | LexerState::InVariable) => {
                // in blocks whitespace is generally ignored, skip it.
                match state
                    .rest
                    .as_bytes()
                    .iter()
                    .position(|&x| !x.is_ascii_whitespace())
                {
                    Some(0) => {}
                    None => {
                        state.advance(state.rest.len());
                        continue;
                    }
                    Some(offset) => {
                        state.advance(offset);
                        continue;
                    }
                }

                // look out for the end of blocks
                if let Some(&LexerState::InBlock) = state.stack.last() {
                    if state.rest.get(..syntax_config.syntax.block_end.len() + 1)
                        == Some(&format!("-{}", syntax_config.syntax.block_end))
                    {
                        state.stack.pop();
                        trim_leading_whitespace = true;
                        state.advance(syntax_config.syntax.block_end.len() + 1);
                        return Some(Ok((Token::BlockEnd, state.span(old_loc))));
                    }
                    if state.rest.get(..syntax_config.syntax.block_end.len())
                        == Some(syntax_config.syntax.block_end.borrow())
                    {
                        state.stack.pop();
                        state.advance(syntax_config.syntax.block_end.len());
                        return Some(Ok((Token::BlockEnd, state.span(old_loc))));
                    }
                } else {
                    if state
                        .rest
                        .get(..syntax_config.syntax.variable_end.len() + 1)
                        == Some(&format!("-{}", syntax_config.syntax.variable_end))
                    {
                        state.stack.pop();
                        state.advance(syntax_config.syntax.variable_end.len() + 1);
                        trim_leading_whitespace = true;
                        return Some(Ok((Token::VariableEnd, state.span(old_loc))));
                    }
                    if state.rest.get(..syntax_config.syntax.variable_end.len())
                        == Some(syntax_config.syntax.variable_end.borrow())
                    {
                        state.stack.pop();
                        state.advance(syntax_config.syntax.variable_end.len());
                        return Some(Ok((Token::VariableEnd, state.span(old_loc))));
                    }
                }

                // two character operators
                let op = match state.rest.as_bytes().get(..2) {
                    Some(b"//") => Some(Token::FloorDiv),
                    Some(b"**") => Some(Token::Pow),
                    Some(b"==") => Some(Token::Eq),
                    Some(b"!=") => Some(Token::Ne),
                    Some(b">=") => Some(Token::Gte),
                    Some(b"<=") => Some(Token::Lte),
                    _ => None,
                };
                if let Some(op) = op {
                    state.advance(2);
                    return Some(Ok((op, state.span(old_loc))));
                }

                // single character operators (and strings)
                let op = match state.rest.as_bytes().get(0) {
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
                        return Some(state.eat_string(b'\''));
                    }
                    Some(b'"') => {
                        return Some(state.eat_string(b'"'));
                    }
                    Some(c) if c.is_ascii_digit() => return Some(state.eat_number()),
                    _ => None,
                };
                if let Some(op) = op {
                    state.advance(1);
                    return Some(Ok((op, state.span(old_loc))));
                }

                return Some(state.eat_identifier());
            }
            None => panic!("empty lexer state"),
        }
    })
}

#[test]
fn test_find_marker() {
    let syntax = SyntaxConfig::default();
    assert!(find_marker("{", &syntax).is_none());
    assert!(find_marker("foo", &syntax).is_none());
    assert!(find_marker("foo {", &syntax).is_none());
    assert_eq!(find_marker("foo {{", &syntax), Some((4, false)));
    assert_eq!(find_marker("foo {{-", &syntax), Some((4, true)));
}

#[test]
#[cfg(feature = "custom_delimiters")]
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

    let syntax_config: SyntaxConfig = syntax.compile().expect("failed to create syntax config");

    assert_eq!(find_marker("%{", &syntax_config), Some((0, false)));
    assert!(find_marker("/", &syntax_config).is_none());
    assert!(find_marker("foo [", &syntax_config).is_none());
    assert_eq!(find_marker("foo /*", &syntax_config), Some((4, false)));
    assert_eq!(find_marker("foo [[-", &syntax_config), Some((4, true)));
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
