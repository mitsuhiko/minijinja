use std::char::decode_utf16;
use std::collections::BTreeMap;
use std::fmt;
use std::iter::{once, repeat};
use std::str::Chars;

use crate::error::{Error, ErrorKind};

#[cfg(test)]
use similar_asserts::assert_eq;

// we target Rust 1.41 and that does not have this macro yet
macro_rules! _matches {
    ($expression:expr, $( $pattern:pat )|+ $( if $guard: expr )? $(,)?) => {
        match $expression {
            $( $pattern )|+ $( if $guard )? => true,
            _ => false
        }
    }
}
pub(crate) use _matches as matches;

pub fn memchr(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|&x| x == needle)
}

pub fn memstr(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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
                            f.write_str(unsafe {
                                std::str::from_utf8_unchecked(&bytes[start..i])
                            })?;
                        }
                        f.write_str($quote)?;
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
                        '"' | '\\' | '/' | '\'' => self.push_char(d)?,
                        'b' => self.push_char('\x08')?,
                        'f' => self.push_char('\x0C')?,
                        'n' => self.push_char('\n')?,
                        'r' => self.push_char('\r')?,
                        't' => self.push_char('\t')?,
                        'u' => {
                            let val = self.parse_u16(&mut char_iter)?;
                            self.push_u16(val)?;
                        }
                        _ => return Err(ErrorKind::BadEscape.into()),
                    },
                }
            } else {
                self.push_char(c)?;
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
