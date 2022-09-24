use std::borrow::Cow;

use crate::compiler::tokens::{Span, Token};
use crate::error::{Error, ErrorKind};
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
    current_line: usize,
    current_col: usize,
}

#[inline(always)]
fn find_marker(a: &str) -> Option<usize> {
    let bytes = a.as_bytes();
    let mut offset = 0;
    loop {
        let idx = match memchr(&bytes[offset..], b'{') {
            Some(idx) => idx,
            None => return None,
        };
        if let Some(b'{' | b'%' | b'#') = bytes.get(offset + idx + 1).copied() {
            return Some(offset + idx);
        }
        offset += idx + 1;
    }
}

fn skip_basic_tag(block_str: &str, name: &str) -> Option<usize> {
    let mut ptr = block_str;

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
    }
    ptr = match ptr.strip_prefix("%}") {
        Some(ptr) => ptr,
        None => return None,
    };

    Some(block_str.len() - ptr.len())
}

impl<'s> TokenizerState<'s> {
    #[inline(always)]
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
    fn loc(&self) -> (usize, usize) {
        (self.current_line, self.current_col)
    }

    #[inline(always)]
    fn span(&self, start: (usize, usize)) -> Span {
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
        let old_loc = self.loc();
        let mut is_float = false;
        let num_len = self
            .rest
            .as_bytes()
            .iter()
            .take_while(|&&c| {
                if !is_float && c == b'.' {
                    is_float = true;
                    true
                } else {
                    c.is_ascii_digit()
                }
            })
            .count();
        let num = self.advance(num_len);
        Ok(if is_float {
            (
                Token::Float(match num.parse::<f64>() {
                    Ok(val) => val,
                    Err(_) => return Err(self.syntax_error("invalid float")),
                }),
                self.span(old_loc),
            )
        } else {
            (
                Token::Int(match num.parse::<i64>() {
                    Ok(val) => val,
                    Err(_) => return Err(self.syntax_error("invalid integer")),
                }),
                self.span(old_loc),
            )
        })
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
                Token::Str(Cow::Owned(match unescape(&s[1..s.len() - 1]) {
                    Ok(unescaped) => unescaped,
                    Err(err) => return Err(err),
                })),
                self.span(old_loc),
            )
        } else {
            (
                Token::Str(Cow::Borrowed(&s[1..s.len() - 1])),
                self.span(old_loc),
            )
        })
    }
}

/// Tokenizes without whitespace handling.
fn tokenize_raw(
    input: &str,
    in_expr: bool,
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

    std::iter::from_fn(move || loop {
        if state.rest.is_empty() || state.failed {
            return None;
        }

        let old_loc = state.loc();
        match state.stack.last() {
            Some(LexerState::Template) => {
                match state.rest.get(..2) {
                    Some("{{") => {
                        let ws = if state.rest.as_bytes().get(2) == Some(&b'-') {
                            state.advance(3);
                            true
                        } else {
                            state.advance(2);
                            false
                        };
                        state.stack.push(LexerState::InVariable);
                        return Some(Ok((Token::VariableStart(ws), state.span(old_loc))));
                    }
                    Some("{%") => {
                        // raw blocks require some special handling.  If we are at the beginning of a raw
                        // block we want to skip everything until {% endraw %} completely ignoring iterior
                        // syntax and emit the entire raw block as TemplateData.
                        if let Some(mut ptr) = skip_basic_tag(&state.rest[2..], "raw") {
                            ptr += 2;
                            while let Some(block) = memstr(&state.rest.as_bytes()[ptr..], b"{%") {
                                ptr += block + 2;
                                if let Some(endraw) = skip_basic_tag(&state.rest[ptr..], "endraw") {
                                    let result = &state.rest[..ptr + endraw];
                                    state.advance(ptr + endraw);
                                    return Some(Ok((
                                        Token::TemplateData(result),
                                        state.span(old_loc),
                                    )));
                                }
                            }
                            return Some(Err(state.syntax_error("unexpected end of raw block")));
                        }

                        let ws = if state.rest.as_bytes().get(2) == Some(&b'-') {
                            state.advance(3);
                            true
                        } else {
                            state.advance(2);
                            false
                        };

                        state.stack.push(LexerState::InBlock);
                        return Some(Ok((Token::BlockStart(ws), state.span(old_loc))));
                    }
                    Some("{#") => {
                        if let Some(comment_end) = memstr(state.rest.as_bytes(), b"#}") {
                            state.advance(comment_end + 2);
                        } else {
                            return Some(Err(state.syntax_error("unexpected end of comment")));
                        }
                    }
                    _ => {}
                }

                let lead = match find_marker(state.rest) {
                    Some(start) => state.advance(start),
                    None => state.advance(state.rest.len()),
                };
                return Some(Ok((Token::TemplateData(lead), state.span(old_loc))));
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
                    if let Some("-%}") = state.rest.get(..3) {
                        state.stack.pop();
                        state.advance(3);
                        return Some(Ok((Token::BlockEnd(true), state.span(old_loc))));
                    }
                    if let Some("%}") = state.rest.get(..2) {
                        state.stack.pop();
                        state.advance(2);
                        return Some(Ok((Token::BlockEnd(false), state.span(old_loc))));
                    }
                } else {
                    if let Some("-}}") = state.rest.get(..3) {
                        state.stack.pop();
                        state.advance(3);
                        return Some(Ok((Token::VariableEnd(true), state.span(old_loc))));
                    }
                    if let Some("}}") = state.rest.get(..2) {
                        state.stack.pop();
                        state.advance(2);
                        return Some(Ok((Token::VariableEnd(false), state.span(old_loc))));
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

                // identifiers
                let ident_len = state
                    .rest
                    .as_bytes()
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
                    .count();
                if ident_len > 0 {
                    let ident = state.advance(ident_len);
                    return Some(Ok((Token::Ident(ident), state.span(old_loc))));
                }

                // syntax error
                return Some(Err(state.syntax_error("unexpected character")));
            }
            None => panic!("empty lexer state"),
        }
    })
}

/// Automatically removes whitespace around blocks.
fn whitespace_filter<'a, I: Iterator<Item = Result<(Token<'a>, Span), Error>>>(
    iter: I,
) -> impl Iterator<Item = Result<(Token<'a>, Span), Error>> {
    let mut iter = iter.peekable();
    let mut remove_leading_ws = false;
    // TODO: this does not update spans
    std::iter::from_fn(move || loop {
        return match iter.next() {
            Some(Ok((Token::TemplateData(mut data), span))) => {
                if remove_leading_ws {
                    remove_leading_ws = false;
                    data = data.trim_start();
                }
                if matches!(
                    iter.peek(),
                    Some(Ok((
                        Token::VariableStart(true) | Token::BlockStart(true),
                        _
                    )))
                ) {
                    data = data.trim_end();
                }
                // if we trim down template data completely, skip to the
                // next token
                if data.is_empty() {
                    continue;
                }
                Some(Ok((Token::TemplateData(data), span)))
            }
            rv @ Some(Ok((Token::VariableEnd(true) | Token::BlockEnd(true), _))) => {
                remove_leading_ws = true;
                rv
            }
            other => {
                remove_leading_ws = false;
                other
            }
        };
    })
}

/// Tokenizes the source.
pub fn tokenize(
    input: &str,
    in_expr: bool,
) -> impl Iterator<Item = Result<(Token<'_>, Span), Error>> {
    whitespace_filter(tokenize_raw(input, in_expr))
}

#[test]
fn test_find_marker() {
    assert!(find_marker("{").is_none());
    assert!(find_marker("foo").is_none());
    assert!(find_marker("foo {").is_none());
    assert_eq!(find_marker("foo {{"), Some(4));
}

#[test]
fn test_is_basic_tag() {
    assert_eq!(skip_basic_tag(" raw %}", "raw"), Some(7));
    assert_eq!(skip_basic_tag(" raw %}", "endraw"), None);
    assert_eq!(skip_basic_tag("  raw  %}", "raw"), Some(9));
    assert_eq!(skip_basic_tag("-  raw  -%}", "raw"), Some(11));
}
