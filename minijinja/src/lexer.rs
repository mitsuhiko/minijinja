use std::borrow::Cow;

use crate::error::{Error, ErrorKind};
use crate::tokens::{Span, Token};
use crate::utils::{matches, memchr, memstr, unescape};

enum LexerState {
    Template,
    InVariable,
    InBlock,
}

fn find_marker(a: &str) -> Option<usize> {
    let bytes = a.as_bytes();
    let mut offset = 0;
    loop {
        let idx = memchr(&bytes[offset..], b'{')?;
        if let Some(b'{') | Some(b'%') | Some(b'#') = bytes.get(offset + idx + 1).copied() {
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

    ptr = ptr.strip_prefix(name)?;

    while let Some(rest) = ptr.strip_prefix(|x: char| x.is_ascii_whitespace()) {
        ptr = rest;
    }
    if let Some(rest) = ptr.strip_prefix('-') {
        ptr = rest;
    }
    ptr = ptr.strip_prefix("%}")?;

    Some(block_str.len() - ptr.len())
}

/// Tokenizes without whitespace handling.
fn tokenize_raw(
    input: &str,
    in_expr: bool,
) -> impl Iterator<Item = Result<(Token<'_>, Span), Error>> {
    let mut rest = input;
    let mut stack = vec![if in_expr {
        LexerState::InVariable
    } else {
        LexerState::Template
    }];
    let mut failed = false;
    let mut current_line = 1;
    let mut current_col = 0;

    macro_rules! syntax_error {
        ($msg:expr) => {{
            failed = true;
            return Some(Err(Error::new(ErrorKind::SyntaxError, $msg)));
        }};
    }

    macro_rules! span {
        ($start:expr) => {{
            let (start_line, start_col) = $start;
            Span {
                start_line,
                start_col,
                end_line: current_line,
                end_col: current_col,
            }
        }};
    }

    macro_rules! loc {
        () => {
            (current_line, current_col)
        };
    }

    macro_rules! advance {
        ($bytes:expr) => {{
            let (skipped, new_rest) = rest.split_at($bytes);
            for c in skipped.chars() {
                match c {
                    '\n' => {
                        current_line += 1;
                        current_col = 0;
                    }
                    _ => current_col += 1,
                }
            }
            rest = new_rest;
            skipped
        }};
    }

    // TODO: this needs to learn how to unescape
    macro_rules! eat_string {
        ($delim:expr) => {{
            let old_loc = loc!();
            let mut escaped = false;
            let mut has_escapes = false;
            let str_len = rest
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
                    (_, $delim) | (_, b'\r') | (_, b'\n') => false,
                    _ => true,
                })
                .count();
            if escaped || rest.as_bytes().get(str_len + 1) != Some(&$delim) {
                syntax_error!("unexpected end of string");
            }
            let s = advance!(str_len + 2);
            if has_escapes {
                return Some(Ok((
                    Token::Str(Cow::Owned(match unescape(&s[1..s.len() - 1]) {
                        Ok(unescaped) => unescaped,
                        Err(err) => return Some(Err(err)),
                    })),
                    span!(old_loc),
                )));
            } else {
                return Some(Ok((
                    Token::Str(Cow::Borrowed(&s[1..s.len() - 1])),
                    span!(old_loc),
                )));
            }
        }};
    }

    macro_rules! eat_number {
        ($neg:expr) => {{
            let old_loc = loc!();
            let mut is_float = false;
            let num_len = rest
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
            let num = advance!(num_len);
            if is_float {
                return Some(Ok((
                    Token::Float(match num.parse::<f64>() {
                        Ok(val) => val * if $neg { -1.0 } else { 1.0 },
                        Err(_) => syntax_error!("invalid float"),
                    }),
                    span!(old_loc),
                )));
            } else {
                return Some(Ok((
                    Token::Int(match num.parse::<i64>() {
                        Ok(val) => val * if $neg { -1 } else { 1 },
                        Err(_) => syntax_error!("invalid integer"),
                    }),
                    span!(old_loc),
                )));
            }
        }};
    }

    std::iter::from_fn(move || loop {
        if rest.is_empty() || failed {
            return None;
        }

        let old_loc = loc!();
        match stack.last() {
            Some(LexerState::Template) => {
                match rest.get(..2) {
                    Some("{{") => {
                        let ws = if rest.as_bytes().get(2) == Some(&b'-') {
                            advance!(3);
                            true
                        } else {
                            advance!(2);
                            false
                        };
                        stack.push(LexerState::InVariable);
                        return Some(Ok((Token::VariableStart(ws), span!(old_loc))));
                    }
                    Some("{%") => {
                        // raw blocks require some special handling.  If we are at the beginning of a raw
                        // block we want to skip everything until {% endraw %} completely ignoring iterior
                        // syntax and emit the entire raw block as TemplateData.
                        if let Some(mut ptr) = skip_basic_tag(&rest[2..], "raw") {
                            ptr += 2;
                            while let Some(block) = memstr(&rest.as_bytes()[ptr..], b"{%") {
                                ptr += block + 2;
                                if let Some(endraw) = skip_basic_tag(&rest[ptr..], "endraw") {
                                    let result = &rest[..ptr + endraw];
                                    advance!(ptr + endraw);
                                    return Some(Ok((Token::TemplateData(result), span!(old_loc))));
                                }
                            }
                            syntax_error!("unexpected end of raw block");
                        }

                        let ws = if rest.as_bytes().get(2) == Some(&b'-') {
                            advance!(3);
                            true
                        } else {
                            advance!(2);
                            false
                        };

                        stack.push(LexerState::InBlock);
                        return Some(Ok((Token::BlockStart(ws), span!(old_loc))));
                    }
                    Some("{#") => {
                        if let Some(comment_end) = memstr(rest.as_bytes(), b"#}") {
                            advance!(comment_end + 2);
                        } else {
                            syntax_error!("unexpected end of comment");
                        }
                    }
                    _ => {}
                }

                let lead = match find_marker(rest) {
                    Some(start) => advance!(start),
                    None => advance!(rest.len()),
                };
                return Some(Ok((Token::TemplateData(lead), span!(old_loc))));
            }
            Some(&LexerState::InBlock) | Some(&LexerState::InVariable) => {
                // in blocks whitespace is generally ignored, skip it.
                match rest
                    .as_bytes()
                    .iter()
                    .position(|&x| !x.is_ascii_whitespace())
                {
                    Some(0) => {}
                    None => {
                        advance!(rest.len());
                        continue;
                    }
                    Some(offset) => {
                        advance!(offset);
                        continue;
                    }
                }

                // look out for the end of blocks
                if let Some(&LexerState::InBlock) = stack.last() {
                    if let Some("-%}") = rest.get(..3) {
                        stack.pop();
                        advance!(3);
                        return Some(Ok((Token::BlockEnd(true), span!(old_loc))));
                    }
                    if let Some("%}") = rest.get(..2) {
                        stack.pop();
                        advance!(2);
                        return Some(Ok((Token::BlockEnd(false), span!(old_loc))));
                    }
                } else {
                    if let Some("-}}") = rest.get(..3) {
                        stack.pop();
                        advance!(3);
                        return Some(Ok((Token::VariableEnd(true), span!(old_loc))));
                    }
                    if let Some("}}") = rest.get(..2) {
                        stack.pop();
                        advance!(2);
                        return Some(Ok((Token::VariableEnd(false), span!(old_loc))));
                    }
                }

                // two character operators
                let op = match rest.as_bytes().get(..2) {
                    Some(b"//") => Some(Token::FloorDiv),
                    Some(b"**") => Some(Token::Pow),
                    Some(b"==") => Some(Token::Eq),
                    Some(b"!=") => Some(Token::Ne),
                    Some(b">=") => Some(Token::Gte),
                    Some(b"<=") => Some(Token::Lte),
                    _ => None,
                };
                if let Some(op) = op {
                    advance!(2);
                    return Some(Ok((op, span!(old_loc))));
                }

                // single character operators (and strings)
                let op = match rest.as_bytes().get(0) {
                    Some(b'+') => Some(Token::Plus),
                    Some(b'-') => {
                        if rest.as_bytes().get(1).map_or(false, |x| x.is_ascii_digit()) {
                            advance!(1);
                            eat_number!(true);
                        }
                        Some(Token::Minus)
                    }
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
                    Some(b'\'') => eat_string!(b'\''),
                    Some(b'"') => eat_string!(b'"'),
                    Some(c) if c.is_ascii_digit() => eat_number!(false),
                    _ => None,
                };
                if let Some(op) = op {
                    advance!(1);
                    return Some(Ok((op, span!(old_loc))));
                }

                // identifiers
                let ident_len = rest
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
                    let ident = advance!(ident_len);
                    return Some(Ok((Token::Ident(ident), span!(old_loc))));
                }

                // syntax error
                syntax_error!("unexpected character");
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
    std::iter::from_fn(move || match iter.next() {
        Some(Ok((Token::TemplateData(mut data), span))) => {
            if remove_leading_ws {
                remove_leading_ws = false;
                data = data.trim_start();
            }
            if matches!(
                iter.peek(),
                Some(Ok((Token::VariableStart(true), _))) | Some(Ok((Token::BlockStart(true), _)))
            ) {
                data = data.trim_end();
            }
            Some(Ok((Token::TemplateData(data), span)))
        }
        rv @ Some(Ok((Token::VariableEnd(true), _)))
        | rv @ Some(Ok((Token::BlockStart(true), _))) => {
            remove_leading_ws = true;
            rv
        }
        other => {
            remove_leading_ws = false;
            other
        }
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
fn test_whitespace_filter() {
    let input = "foo {{- bar -}} baz {{ blah }} blub";
    let tokens: Result<Vec<_>, _> = tokenize(input, false).collect();
    let tokens = tokens.unwrap().into_iter().map(|x| x.0).collect::<Vec<_>>();
    insta::assert_debug_snapshot!(&tokens, @r###"
    [
        TEMPLATE_DATA("foo"),
        VARIABLE_START(true),
        IDENT(bar),
        VARIABLE_END(true),
        TEMPLATE_DATA("baz "),
        VARIABLE_START(false),
        IDENT(blah),
        VARIABLE_END(false),
        TEMPLATE_DATA(" blub"),
    ]
    "###);
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
