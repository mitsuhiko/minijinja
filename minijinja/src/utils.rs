use std::char::decode_utf16;
use std::collections::BTreeMap;
use std::fmt;
use std::iter::{once, repeat};
use std::str::Chars;

use crate::error::{Error, ErrorKind};
use crate::value::{OwnedValueIterator, StringType, Value, ValueKind, ValueRepr};
use crate::Output;

#[cfg(test)]
use similar_asserts::assert_eq;

/// internal marker to seal up some trait methods
pub struct SealedMarker;

pub fn memchr(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|&x| x == needle)
}

pub fn memstr(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn write_with_html_escaping(out: &mut Output, value: &Value) -> fmt::Result {
    if matches!(
        value.kind(),
        ValueKind::Undefined | ValueKind::None | ValueKind::Bool | ValueKind::Number
    ) {
        write!(out, "{value}")
    } else if let Some(s) = value.as_str() {
        write!(out, "{}", HtmlEscape(s))
    } else {
        write!(out, "{}", HtmlEscape(&value.to_string()))
    }
}

fn invalid_autoescape(name: &str) -> Result<(), Error> {
    Err(Error::new(
        ErrorKind::InvalidOperation,
        format!("Default formatter does not know how to format to custom format '{name}'"),
    ))
}

#[inline(always)]
pub fn write_escaped(
    out: &mut Output,
    auto_escape: AutoEscape,
    value: &Value,
) -> Result<(), Error> {
    // common case of safe strings or strings without auto escaping
    if let ValueRepr::String(ref s, ty) = value.0 {
        if matches!(ty, StringType::Safe) || matches!(auto_escape, AutoEscape::None) {
            return out.write_str(s).map_err(Error::from);
        }
    }

    match auto_escape {
        AutoEscape::None => write!(out, "{value}").map_err(Error::from),
        AutoEscape::Html => write_with_html_escaping(out, value).map_err(Error::from),
        #[cfg(feature = "json")]
        AutoEscape::Json => {
            let value = ok!(serde_json::to_string(&value).map_err(|err| {
                Error::new(ErrorKind::BadSerialization, "unable to format to JSON").with_source(err)
            }));
            write!(out, "{value}").map_err(Error::from)
        }
        AutoEscape::Custom(name) => invalid_autoescape(name),
    }
}

/// Controls the autoescaping behavior.
///
/// For more information see
/// [`set_auto_escape_callback`](crate::Environment::set_auto_escape_callback).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AutoEscape {
    /// Do not apply auto escaping.
    None,
    /// Use HTML auto escaping rules.
    ///
    /// Any value will be converted into a string and the following characters
    /// will be escaped in ways compatible to XML and HTML: `<`, `>`, `&`, `"`,
    /// `'`, and `/`.
    Html,
    /// Use escaping rules suitable for JSON/JavaScript or YAML.
    ///
    /// Any value effectively ends up being serialized to JSON upon printing.  The
    /// serialized values will be compatible with JavaScript and YAML as well.
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    Json,
    /// A custom auto escape format.
    ///
    /// The default formatter does not know how to deal with a custom escaping
    /// format and would error.  The use of these requires a custom formatter.
    /// See [`set_formatter`](crate::Environment::set_formatter).
    Custom(&'static str),
}

/// Defines the behavior of undefined values in the engine.
///
/// At present there are three types of behaviors available which mirror the behaviors
/// that Jinja2 provides out of the box.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UndefinedBehavior {
    /// The default, somewhat lenient undefined behavior.
    ///
    /// * **printing:** allowed (returns empty string)
    /// * **iteration:** allowed (returns empty array)
    /// * **attribute access of undefined values:** fails
    Lenient,
    /// Like `Lenient`, but also allows chaining of undefined lookups.
    ///
    /// * **printing:** allowed (returns empty string)
    /// * **iteration:** allowed (returns empty array)
    /// * **attribute access of undefined values:** allowed (returns [`undefined`](Value::UNDEFINED))
    Chainable,
    /// Complains very quickly about undefined values.
    ///
    /// * **printing:** fails
    /// * **iteration:** fails
    /// * **attribute access of undefined values:** fails
    Strict,
}

impl Default for UndefinedBehavior {
    fn default() -> UndefinedBehavior {
        UndefinedBehavior::Lenient
    }
}

impl UndefinedBehavior {
    /// Utility method used in the engine to determine what to do when an undefined is
    /// encountered.
    ///
    /// The flag indicates if this is the first or second level of undefined value.  If
    /// `parent_was_undefined` is set to `true`, the undefined was created by looking up
    /// a missing attribute on an undefined value.  If `false` the undefined was created by
    /// looing up a missing attribute on a defined value.
    pub(crate) fn handle_undefined(self, parent_was_undefined: bool) -> Result<Value, Error> {
        match (self, parent_was_undefined) {
            (UndefinedBehavior::Lenient, false) | (UndefinedBehavior::Chainable, _) => {
                Ok(Value::UNDEFINED)
            }
            (UndefinedBehavior::Lenient, true) | (UndefinedBehavior::Strict, _) => {
                Err(Error::from(ErrorKind::UndefinedError))
            }
        }
    }

    /// Tries to iterate over a value while handling the undefined value.
    ///
    /// If the value is undefined, then iteration fails if the behavior is set to strict,
    /// otherwise it succeeds with an empty iteration.  This is also internally used in the
    /// engine to convert values to lists.
    pub(crate) fn try_iter(self, value: Value) -> Result<OwnedValueIterator, Error> {
        if matches!(self, UndefinedBehavior::Strict) && value.is_undefined() {
            Err(Error::from(ErrorKind::UndefinedError))
        } else {
            value.try_iter_owned()
        }
    }
}

/// Helper to HTML escape a string.
pub struct HtmlEscape<'a>(pub &'a str);

impl<'a> fmt::Display for HtmlEscape<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "v_htmlescape")]
        {
            fmt::Display::fmt(&v_htmlescape::escape(self.0), f)
        }
        // this is taken from askama-escape
        #[cfg(not(feature = "v_htmlescape"))]
        {
            let bytes = self.0.as_bytes();
            let mut start = 0;

            for (i, b) in bytes.iter().enumerate() {
                macro_rules! escaping_body {
                    ($quote:expr) => {{
                        if start < i {
                            ok!(f.write_str(unsafe {
                                std::str::from_utf8_unchecked(&bytes[start..i])
                            }));
                        }
                        ok!(f.write_str($quote));
                        start = i + 1;
                    }};
                }
                if b.wrapping_sub(b'"') <= b'>' - b'"' {
                    match *b {
                        b'<' => escaping_body!("&lt;"),
                        b'>' => escaping_body!("&gt;"),
                        b'&' => escaping_body!("&amp;"),
                        b'"' => escaping_body!("&quot;"),
                        b'\'' => escaping_body!("&#x27;"),
                        b'/' => escaping_body!("&#x2f;"),
                        _ => (),
                    }
                }
            }

            if start < bytes.len() {
                f.write_str(unsafe { std::str::from_utf8_unchecked(&bytes[start..]) })
            } else {
                Ok(())
            }
        }
    }
}

struct Unescaper {
    out: String,
    pending_surrogate: u16,
}

impl Unescaper {
    fn unescape(mut self, s: &str) -> Result<String, Error> {
        let mut char_iter = s.chars();

        while let Some(c) = char_iter.next() {
            if c == '\\' {
                match char_iter.next() {
                    None => return Err(ErrorKind::BadEscape.into()),
                    Some(d) => match d {
                        '"' | '\\' | '/' | '\'' => ok!(self.push_char(d)),
                        'b' => ok!(self.push_char('\x08')),
                        'f' => ok!(self.push_char('\x0C')),
                        'n' => ok!(self.push_char('\n')),
                        'r' => ok!(self.push_char('\r')),
                        't' => ok!(self.push_char('\t')),
                        'u' => {
                            let val = ok!(self.parse_u16(&mut char_iter));
                            ok!(self.push_u16(val));
                        }
                        _ => return Err(ErrorKind::BadEscape.into()),
                    },
                }
            } else {
                ok!(self.push_char(c));
            }
        }

        if self.pending_surrogate != 0 {
            Err(ErrorKind::BadEscape.into())
        } else {
            Ok(self.out)
        }
    }

    fn parse_u16(&self, chars: &mut Chars) -> Result<u16, Error> {
        let hexnum = chars.chain(repeat('\0')).take(4).collect::<String>();
        u16::from_str_radix(&hexnum, 16).map_err(|_| ErrorKind::BadEscape.into())
    }

    fn push_u16(&mut self, c: u16) -> Result<(), Error> {
        match (self.pending_surrogate, (0xD800..=0xDFFF).contains(&c)) {
            (0, false) => match decode_utf16(once(c)).next() {
                Some(Ok(c)) => self.out.push(c),
                _ => return Err(ErrorKind::BadEscape.into()),
            },
            (_, false) => return Err(ErrorKind::BadEscape.into()),
            (0, true) => self.pending_surrogate = c,
            (prev, true) => match decode_utf16(once(prev).chain(once(c))).next() {
                Some(Ok(c)) => {
                    self.out.push(c);
                    self.pending_surrogate = 0;
                }
                _ => return Err(ErrorKind::BadEscape.into()),
            },
        }
        Ok(())
    }

    fn push_char(&mut self, c: char) -> Result<(), Error> {
        if self.pending_surrogate != 0 {
            Err(ErrorKind::BadEscape.into())
        } else {
            self.out.push(c);
            Ok(())
        }
    }
}

/// Un-escape a string, following JSON rules.
pub fn unescape(s: &str) -> Result<String, Error> {
    Unescaper {
        out: String::new(),
        pending_surrogate: 0,
    }
    .unescape(s)
}

pub struct BTreeMapKeysDebug<'a, K: fmt::Debug, V>(pub &'a BTreeMap<K, V>);

impl<'a, K: fmt::Debug, V> fmt::Debug for BTreeMapKeysDebug<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.0.iter().map(|x| x.0)).finish()
    }
}

pub struct OnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> OnDrop<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        self.0.take().unwrap()();
    }
}

#[test]
fn test_html_escape() {
    let input = "<>&\"'/";
    let output = HtmlEscape(input).to_string();
    assert_eq!(output, "&lt;&gt;&amp;&quot;&#x27;&#x2f;");
}

#[test]
fn test_unescape() {
    assert_eq!(unescape(r"foo\u2603bar").unwrap(), "foo\u{2603}bar");
    assert_eq!(unescape(r"\t\b\f\r\n\\\/").unwrap(), "\t\x08\x0c\r\n\\/");
    assert_eq!(unescape("foobarbaz").unwrap(), "foobarbaz");
    assert_eq!(unescape(r"\ud83d\udca9").unwrap(), "💩");
}
