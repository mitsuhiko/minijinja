//! A small JSON5 parser (<https://spec.json5.org/>).
//!
//! This parses directly into a minimal tree which is then converted into a
//! MiniJinja [`Value`].  This exists so that the CLI does not need to pull in
//! a parser generator and its dependency tree just to support JSON5.
use std::fmt;

use minijinja::value::{Serde, Value};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

#[derive(Debug)]
pub struct Error {
    msg: String,
    line: usize,
    col: usize,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at line {} column {}", self.msg, self.line, self.col)
    }
}

impl std::error::Error for Error {}

enum Node {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    I128(i128),
    U128(u128),
    F64(f64),
    String(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Node::Null => serializer.serialize_unit(),
            Node::Bool(v) => serializer.serialize_bool(v),
            Node::I64(v) => serializer.serialize_i64(v),
            Node::U64(v) => serializer.serialize_u64(v),
            Node::I128(v) => serializer.serialize_i128(v),
            Node::U128(v) => serializer.serialize_u128(v),
            Node::F64(v) => serializer.serialize_f64(v),
            Node::String(ref v) => serializer.serialize_str(v),
            Node::Array(ref items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Node::Object(ref items) => {
                let mut map = serializer.serialize_map(Some(items.len()))?;
                for (key, value) in items {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

/// Parses a JSON5 document into a value.
pub fn from_slice(input: &[u8]) -> Result<Value, Error> {
    let input = match std::str::from_utf8(input) {
        Ok(input) => input,
        Err(err) => {
            let parser = Parser {
                input: std::str::from_utf8(&input[..err.valid_up_to()]).unwrap(),
                pos: err.valid_up_to(),
            };
            return Err(parser.error("invalid utf-8"));
        }
    };
    let mut parser = Parser { input, pos: 0 };
    let node = parser.parse_document()?;
    Ok(Value::from(Serde(node)))
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

const MAX_DEPTH: usize = 512;

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_whitespace(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

fn is_ident_start(c: char) -> bool {
    c == '$' || c == '_' || c.is_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    is_ident_start(c) || c.is_alphanumeric() || c == '\u{200c}' || c == '\u{200d}'
}

impl<'a> Parser<'a> {
    fn error(&self, msg: impl Into<String>) -> Error {
        let before = &self.input[..self.pos.min(self.input.len())];
        let line = before.matches('\n').count() + 1;
        let col = before
            .rfind('\n')
            .map_or(before, |idx| &before[idx + 1..])
            .chars()
            .count()
            + 1;
        Error {
            msg: msg.into(),
            line,
            col,
        }
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) -> Result<(), Error> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(self.unexpected(&format!("expected '{}'", c)))
        }
    }

    fn unexpected(&self, expected: &str) -> Error {
        match self.peek() {
            Some(c) => self.error(format!("unexpected character {:?}, {}", c, expected)),
            None => self.error(format!("unexpected end of input, {}", expected)),
        }
    }

    fn skip_ws(&mut self) -> Result<(), Error> {
        loop {
            let rest = self.rest();
            if rest.starts_with("//") {
                let len = rest.find(is_line_terminator).unwrap_or(rest.len());
                self.pos += len;
            } else if let Some(comment) = rest.strip_prefix("/*") {
                match comment.find("*/") {
                    Some(idx) => self.pos += idx + 4,
                    None => return Err(self.error("unterminated block comment")),
                }
            } else if self.peek().is_some_and(is_whitespace) {
                self.bump();
            } else {
                return Ok(());
            }
        }
    }

    fn parse_document(&mut self) -> Result<Node, Error> {
        self.skip_ws()?;
        let node = self.parse_value(0)?;
        self.skip_ws()?;
        if self.peek().is_some() {
            return Err(self.unexpected("expected end of input"));
        }
        Ok(node)
    }

    fn parse_value(&mut self, depth: usize) -> Result<Node, Error> {
        if depth > MAX_DEPTH {
            return Err(self.error("recursion limit exceeded"));
        }
        match self.peek() {
            Some('{') => self.parse_object(depth),
            Some('[') => self.parse_array(depth),
            Some(q @ ('"' | '\'')) => {
                self.bump();
                self.parse_string(q).map(Node::String)
            }
            Some(c) if c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'I' | 'N') => {
                self.parse_number()
            }
            Some(c) if is_ident_start(c) => {
                let start = self.pos;
                let ident = self.parse_identifier()?;
                match ident.as_str() {
                    "null" => Ok(Node::Null),
                    "true" => Ok(Node::Bool(true)),
                    "false" => Ok(Node::Bool(false)),
                    _ => {
                        self.pos = start;
                        Err(self.error(format!("unexpected identifier '{}'", ident)))
                    }
                }
            }
            _ => Err(self.unexpected("expected value")),
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Node, Error> {
        self.expect('{')?;
        let mut items = Vec::new();
        loop {
            self.skip_ws()?;
            if self.eat('}') {
                break;
            }
            let key = match self.peek() {
                Some(q @ ('"' | '\'')) => {
                    self.bump();
                    self.parse_string(q)?
                }
                Some(c) if is_ident_start(c) || c == '\\' => self.parse_identifier()?,
                _ => return Err(self.unexpected("expected object key")),
            };
            self.skip_ws()?;
            self.expect(':')?;
            self.skip_ws()?;
            let value = self.parse_value(depth + 1)?;
            items.push((key, value));
            self.skip_ws()?;
            if !self.eat(',') {
                self.skip_ws()?;
                self.expect('}')?;
                break;
            }
        }
        Ok(Node::Object(items))
    }

    fn parse_array(&mut self, depth: usize) -> Result<Node, Error> {
        self.expect('[')?;
        let mut items = Vec::new();
        loop {
            self.skip_ws()?;
            if self.eat(']') {
                break;
            }
            items.push(self.parse_value(depth + 1)?);
            self.skip_ws()?;
            if !self.eat(',') {
                self.skip_ws()?;
                self.expect(']')?;
                break;
            }
        }
        Ok(Node::Array(items))
    }

    fn parse_identifier(&mut self) -> Result<String, Error> {
        let mut rv = String::new();
        loop {
            let start = self.pos;
            let c = match self.peek() {
                Some('\\') => {
                    self.bump();
                    if !self.eat('u') {
                        return Err(self.unexpected("expected 'u'"));
                    }
                    self.parse_unicode_escape()?
                }
                Some(c) => {
                    self.bump();
                    c
                }
                None => break,
            };
            let valid = if rv.is_empty() {
                is_ident_start(c)
            } else {
                is_ident_continue(c)
            };
            if !valid {
                self.pos = start;
                break;
            }
            rv.push(c);
        }
        if rv.is_empty() {
            return Err(self.unexpected("expected identifier"));
        }
        Ok(rv)
    }

    fn parse_hex(&mut self, digits: usize) -> Result<u32, Error> {
        let mut rv = 0;
        for _ in 0..digits {
            match self.peek().and_then(|c| c.to_digit(16)) {
                Some(d) => {
                    self.bump();
                    rv = rv * 16 + d;
                }
                None => return Err(self.unexpected("expected hex digit")),
            }
        }
        Ok(rv)
    }

    fn parse_unicode_escape(&mut self) -> Result<char, Error> {
        let start = self.pos;
        let hi = self.parse_hex(4)?;
        let code = if (0xd800..0xdc00).contains(&hi) {
            if !self.rest().starts_with("\\u") {
                self.pos = start;
                return Err(self.error("unpaired surrogate in unicode escape"));
            }
            self.pos += 2;
            let lo = self.parse_hex(4)?;
            if !(0xdc00..0xe000).contains(&lo) {
                self.pos = start;
                return Err(self.error("invalid surrogate pair in unicode escape"));
            }
            0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
        } else {
            hi
        };
        char::from_u32(code).ok_or_else(|| {
            self.pos = start;
            self.error("invalid unicode escape")
        })
    }

    fn parse_string(&mut self, quote: char) -> Result<String, Error> {
        let mut rv = String::new();
        loop {
            let c = match self.bump() {
                Some(c) => c,
                None => return Err(self.error("unterminated string")),
            };
            match c {
                c if c == quote => return Ok(rv),
                '\\' => {
                    let c = match self.bump() {
                        Some(c) => c,
                        None => return Err(self.error("unterminated string")),
                    };
                    match c {
                        'b' => rv.push('\u{8}'),
                        'f' => rv.push('\u{c}'),
                        'n' => rv.push('\n'),
                        'r' => rv.push('\r'),
                        't' => rv.push('\t'),
                        'v' => rv.push('\u{b}'),
                        '0' if !self.peek().is_some_and(|c| c.is_ascii_digit()) => rv.push('\0'),
                        'x' => rv.push(char::from_u32(self.parse_hex(2)?).unwrap()),
                        'u' => rv.push(self.parse_unicode_escape()?),
                        '\r' => {
                            self.eat('\n');
                        }
                        '\n' | '\u{2028}' | '\u{2029}' => {}
                        '1'..='9' | '0' => {
                            self.pos -= 1;
                            return Err(self.error("invalid escape sequence"));
                        }
                        c => rv.push(c),
                    }
                }
                '\n' | '\r' => {
                    self.pos -= 1;
                    return Err(self.error("unescaped line break in string"));
                }
                c => rv.push(c),
            }
        }
    }

    fn parse_number(&mut self) -> Result<Node, Error> {
        let start = self.pos;
        let negative = match self.peek() {
            Some('-') => {
                self.bump();
                true
            }
            Some('+') => {
                self.bump();
                false
            }
            _ => false,
        };
        let sign = if negative { -1.0 } else { 1.0 };

        let rest = self.rest();
        let node = if rest.starts_with("Infinity") {
            self.pos += "Infinity".len();
            Node::F64(sign * f64::INFINITY)
        } else if rest.starts_with("NaN") {
            self.pos += "NaN".len();
            Node::F64(f64::NAN)
        } else if rest.starts_with("0x") || rest.starts_with("0X") {
            self.pos += 2;
            let digits_start = self.pos;
            while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                self.bump();
            }
            let digits = &self.input[digits_start..self.pos];
            if digits.is_empty() {
                return Err(self.unexpected("expected hex digit"));
            }
            let value = u128::from_str_radix(digits, 16).map_err(|_| {
                self.pos = start;
                self.error("hex number out of range")
            })?;
            if negative {
                if value > i128::MAX as u128 + 1 {
                    self.pos = start;
                    return Err(self.error("hex number out of range"));
                }
                int_node((value as i128).wrapping_neg())
            } else {
                uint_node(value)
            }
        } else {
            let mut is_float = false;
            let int_start = self.pos;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
            let int_digits = self.pos - int_start;
            if int_digits > 1 && self.input[int_start..].starts_with('0') {
                self.pos = start;
                return Err(self.error("leading zeros are not allowed"));
            }
            let mut frac_digits = 0;
            if self.eat('.') {
                is_float = true;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.bump();
                    frac_digits += 1;
                }
            }
            if int_digits == 0 && frac_digits == 0 {
                return Err(self.unexpected("expected digit"));
            }
            if self.eat('e') || self.eat('E') {
                is_float = true;
                if !self.eat('+') {
                    self.eat('-');
                }
                let exp_start = self.pos;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.bump();
                }
                if exp_start == self.pos {
                    return Err(self.unexpected("expected exponent"));
                }
            }

            let text = self.input[start..self.pos].trim_start_matches('+');
            if is_float {
                let value: f64 = text.parse().map_err(|_| self.error("invalid number"))?;
                if !value.is_finite() {
                    self.pos = start;
                    return Err(self.error("number out of range"));
                }
                Node::F64(value)
            } else if let Ok(value) = text.parse::<i128>() {
                int_node(value)
            } else if let Ok(value) = text.parse::<u128>() {
                uint_node(value)
            } else {
                Node::F64(text.parse().map_err(|_| self.error("invalid number"))?)
            }
        };

        if self.peek().is_some_and(is_ident_continue) {
            return Err(self.unexpected("expected end of number"));
        }
        Ok(node)
    }
}

fn int_node(value: i128) -> Node {
    if let Ok(value) = i64::try_from(value) {
        Node::I64(value)
    } else if let Ok(value) = u64::try_from(value) {
        Node::U64(value)
    } else {
        Node::I128(value)
    }
}

fn uint_node(value: u128) -> Node {
    if let Ok(value) = i128::try_from(value) {
        int_node(value)
    } else {
        Node::U128(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> String {
        match from_slice(s.as_bytes()) {
            Ok(value) if f64::try_from(value.clone()).is_ok_and(|x| !x.is_finite()) => {
                value.to_string()
            }
            Ok(value) => serde_json::to_string(&value).unwrap(),
            Err(err) => format!("error: {}", err),
        }
    }

    #[test]
    fn test_basics() {
        assert_eq!(parse("null"), "null");
        assert_eq!(parse("true"), "true");
        assert_eq!(parse(" false "), "false");
        assert_eq!(parse("[1, 2, 3,]"), "[1,2,3]");
        assert_eq!(parse("[]"), "[]");
        assert_eq!(parse("{}"), "{}");
        assert_eq!(
            parse("{a: 1, 'b': 2, \"c\": 3, $d_e: 4,}"),
            r#"{"a":1,"b":2,"c":3,"$d_e":4}"#
        );
        assert_eq!(parse("{a: 1, a: 2}"), r#"{"a":2}"#);
        assert_eq!(parse("{\\u0061b: 1}"), r#"{"ab":1}"#);
    }

    #[test]
    fn test_comments() {
        assert_eq!(
            parse("// comment\n/* block\n comment */{a: /* x */ 1} // end"),
            r#"{"a":1}"#
        );
        assert_eq!(
            parse("/* foo"),
            "error: unterminated block comment at line 1 column 1"
        );
    }

    #[test]
    fn test_numbers() {
        assert_eq!(parse("42"), "42");
        assert_eq!(parse("-42"), "-42");
        assert_eq!(parse("+42"), "42");
        assert_eq!(parse("-0"), "0");
        assert_eq!(parse("1.5"), "1.5");
        assert_eq!(parse(".5"), "0.5");
        assert_eq!(parse("5."), "5.0");
        assert_eq!(parse("1e3"), "1000.0");
        assert_eq!(parse("1E-3"), "0.001");
        assert_eq!(parse("0x1F"), "31");
        assert_eq!(parse("-0X10"), "-16");
        assert_eq!(parse("Infinity"), "inf");
        assert_eq!(parse("-Infinity"), "-inf");
        assert_eq!(parse("NaN"), "NaN");
        assert_eq!(parse("18446744073709551615"), "18446744073709551615");
        assert_eq!(parse("-9223372036854775808"), "-9223372036854775808");
        assert_eq!(
            parse("340282366920938463463374607431768211455"),
            "340282366920938463463374607431768211455"
        );
        assert_eq!(
            parse("1e400"),
            "error: number out of range at line 1 column 1"
        );
        assert_eq!(
            parse("01"),
            "error: leading zeros are not allowed at line 1 column 1"
        );
        assert_eq!(
            parse("1a"),
            "error: unexpected character 'a', expected end of number at line 1 column 2"
        );
    }

    #[test]
    fn test_strings() {
        assert_eq!(parse(r#"'it\'s'"#), r#""it's""#);
        assert_eq!(parse(r#""a\"b""#), r#""a\"b""#);
        assert_eq!(parse("'line\\\ncont'"), r#""linecont""#);
        assert_eq!(parse(r#"'\x41\u00e4\0'"#), r#""Aä\u0000""#);
        assert_eq!(parse(r#""\ud83d\ude00""#), r#""😀""#);
        assert_eq!(parse(r#""\q""#), r#""q""#);
        assert_eq!(parse(r#""\v\b\f""#), r#""\u000b\b\f""#);
        assert_eq!(
            parse(r#""\ud83d""#),
            "error: unpaired surrogate in unicode escape at line 1 column 4"
        );
        assert_eq!(
            parse("'a\nb'"),
            "error: unescaped line break in string at line 1 column 3"
        );
        assert_eq!(
            parse("'abc"),
            "error: unterminated string at line 1 column 5"
        );
    }

    #[test]
    fn test_errors() {
        assert_eq!(
            parse("{a: 1} junk"),
            "error: unexpected character 'j', expected end of input at line 1 column 8"
        );
        assert_eq!(
            parse("{\n  a: undefined\n}"),
            "error: unexpected identifier 'undefined' at line 2 column 6"
        );
        assert_eq!(
            parse("[1 2]"),
            "error: unexpected character '2', expected ']' at line 1 column 4"
        );
        assert_eq!(
            parse(""),
            "error: unexpected end of input, expected value at line 1 column 1"
        );
    }
}
