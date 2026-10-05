//! Built-in JSON serialization of values.
//!
//! This is used by the `tojson` filter and the JSON auto escaping.  It does
//! not depend on any serialization library so that the `json` feature is
//! independent of serde.
//!
//! The output is the same as the one of `deser-json` (and recent versions of
//! `serde_json`): floats are formatted like `zmij` does it.  With the
//! `speedups` feature `zmij` and `itoa` are used to format numbers, without
//! it a fallback based on the standard library produces the same text.
use crate::error::{Error, ErrorKind};
use crate::value::{checked, ObjectRepr, Value, ValueRepr};

/// The layout of the generated JSON.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(not(feature = "builtins"), allow(dead_code))]
pub(crate) enum JsonStyle {
    /// No whitespace at all (`[1,2]`).
    Compact,
    /// A space after separators (`[1, 2]`).
    Spaced,
    /// One item per line, indented with the given number of spaces.
    Pretty(usize),
}

/// Serializes a value to JSON.
///
/// If `html_safe` is set, the characters `<`, `>`, `&` and `'` are escaped
/// in strings so that the output can be embedded in HTML (including script
/// tags and single quoted attributes).
pub(crate) fn to_json(value: &Value, style: JsonStyle, html_safe: bool) -> Result<String, Error> {
    let mut writer = JsonWriter {
        out: String::new(),
        style,
        html_safe,
        depth: 0,
    };
    ok!(writer.write_value(value));
    Ok(writer.out)
}

struct JsonWriter {
    out: String,
    style: JsonStyle,
    html_safe: bool,
    depth: usize,
}

impl JsonWriter {
    fn write_value(&mut self, value: &Value) -> Result<(), Error> {
        match value.0 {
            ValueRepr::None | ValueRepr::Undefined(..) => self.out.push_str("null"),
            ValueRepr::Invalid(ref err) => return Err(err.internal_clone()),
            ValueRepr::Bool(b) => self.out.push_str(if b { "true" } else { "false" }),
            ValueRepr::U64(v) => write_int(&mut self.out, v),
            ValueRepr::I64(v) => write_int(&mut self.out, v),
            ValueRepr::U128(v) => write_int(&mut self.out, v.0),
            ValueRepr::I128(v) => write_int(&mut self.out, v.0),
            ValueRepr::F64(v) => {
                if v.is_finite() {
                    write_finite_f64(&mut self.out, v);
                } else {
                    self.out.push_str("null");
                }
            }
            ValueRepr::String(ref s, _) => self.write_str(s),
            ValueRepr::SmallStr(ref s) => self.write_str(s.as_str()),
            ValueRepr::Bytes(ref b) => {
                self.begin_container('[');
                for (idx, byte) in b.iter().enumerate() {
                    self.begin_item(idx == 0);
                    write_int(&mut self.out, *byte);
                }
                self.end_container(']', b.is_empty());
            }
            ValueRepr::Object(ref obj) => match obj.repr() {
                ObjectRepr::Plain => self.write_str(&obj.to_string()),
                ObjectRepr::Seq | ObjectRepr::Iterable => {
                    self.begin_container('[');
                    let mut empty = true;
                    if let Some(iter) = obj.try_iter() {
                        for item in checked(iter) {
                            let item = ok!(item);
                            self.begin_item(empty);
                            empty = false;
                            ok!(self.write_value(&item));
                        }
                    }
                    self.end_container(']', empty);
                }
                ObjectRepr::Map => {
                    self.begin_container('{');
                    let mut empty = true;
                    if let Some(iter) = obj.try_iter_pairs() {
                        for pair in checked(iter) {
                            let (key, value) = ok!(pair);
                            self.begin_item(empty);
                            empty = false;
                            ok!(self.write_key(&key));
                            self.out.push_str(match self.style {
                                JsonStyle::Compact => ":",
                                JsonStyle::Spaced | JsonStyle::Pretty(_) => ": ",
                            });
                            ok!(self.write_value(&value));
                        }
                    }
                    self.end_container('}', empty);
                }
            },
        }
        Ok(())
    }

    /// Writes an object key.  Like in JSON keys must be strings, numbers and
    /// booleans are converted into strings.
    fn write_key(&mut self, key: &Value) -> Result<(), Error> {
        match key.0 {
            ValueRepr::String(ref s, _) => self.write_str(s),
            ValueRepr::SmallStr(ref s) => self.write_str(s.as_str()),
            ValueRepr::Object(ref obj) if matches!(obj.repr(), ObjectRepr::Plain) => {
                self.write_str(&obj.to_string())
            }
            ValueRepr::Invalid(ref err) => return Err(err.internal_clone()),
            ValueRepr::Bool(_)
            | ValueRepr::U64(_)
            | ValueRepr::I64(_)
            | ValueRepr::U128(_)
            | ValueRepr::I128(_) => {
                self.out.push('"');
                ok!(self.write_value(key));
                self.out.push('"');
            }
            ValueRepr::F64(v) if v.is_finite() => {
                self.out.push('"');
                write_finite_f64(&mut self.out, v);
                self.out.push('"');
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::BadSerialization,
                    format!("cannot use value of type {} as JSON key", key.kind()),
                ))
            }
        }
        Ok(())
    }

    fn begin_container(&mut self, open: char) {
        self.depth += 1;
        self.out.push(open);
    }

    fn begin_item(&mut self, first: bool) {
        match self.style {
            JsonStyle::Compact => {
                if !first {
                    self.out.push(',');
                }
            }
            JsonStyle::Spaced => {
                if !first {
                    self.out.push_str(", ");
                }
            }
            JsonStyle::Pretty(indent) => {
                self.out.push_str(if first { "\n" } else { ",\n" });
                self.write_indent(indent);
            }
        }
    }

    fn end_container(&mut self, close: char, empty: bool) {
        self.depth -= 1;
        if let JsonStyle::Pretty(indent) = self.style {
            if !empty {
                self.out.push('\n');
                self.write_indent(indent);
            }
        }
        self.out.push(close);
    }

    fn write_indent(&mut self, indent: usize) {
        for _ in 0..indent * self.depth {
            self.out.push(' ');
        }
    }

    /// Writes a JSON string literal.
    #[inline]
    fn write_str(&mut self, value: &str) {
        if self.html_safe {
            write_escaped_str::<true>(&mut self.out, value)
        } else {
            write_escaped_str::<false>(&mut self.out, value)
        }
    }
}

/// Writes a JSON string literal.  With `HTML` HTML special characters are
/// escaped in addition.
#[inline]
fn write_escaped_str<const HTML: bool>(out: &mut String, value: &str) {
    let next = find_escape::<HTML>(value.as_bytes(), 0);
    out.reserve(value.len() + 2);
    out.push('"');
    if next == value.len() {
        out.push_str(value);
    } else {
        write_escaped_str_slow::<HTML>(out, value, next);
    }
    out.push('"');
}

#[inline(never)]
fn write_escaped_str_slow<const HTML: bool>(out: &mut String, value: &str, mut next: usize) {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
    let bytes = value.as_bytes();
    let mut start = 0;
    loop {
        out.push_str(&value[start..next]);
        if next == bytes.len() {
            break;
        }
        let byte = bytes[next];
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\x08' => out.push_str("\\b"),
            b'\x0c' => out.push_str("\\f"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            _ => {
                // control characters and (in HTML mode) <, >, & and '
                out.push_str("\\u00");
                out.push(HEX_DIGITS[(byte >> 4) as usize] as char);
                out.push(HEX_DIGITS[(byte & 0xf) as usize] as char);
            }
        }
        start = next + 1;
        next = find_escape::<HTML>(bytes, start);
    }
}

/// Returns `true` if the byte needs escaping.
#[inline(always)]
fn needs_escape<const HTML: bool>(byte: u8) -> bool {
    byte < 0x20
        || byte == b'"'
        || byte == b'\\'
        || (HTML && matches!(byte, b'<' | b'>' | b'&' | b'\''))
}

const ONE_BYTES: u64 = u64::MAX / 255;
const SPACES: u64 = ONE_BYTES * 0x20;

/// Flags the bytes in a word (in little endian order) which need escaping.
///
/// This is the classic "has zero byte" trick applied to control characters
/// and the characters that need escaping.  Only the lowest flagged byte is
/// exact, bytes above it might be flagged falsely.
#[inline(always)]
fn escape_mask<const HTML: bool>(chars: u64) -> u64 {
    #[inline(always)]
    fn contains(chars: u64, byte: u8) -> u64 {
        let x = chars ^ (ONE_BYTES * u64::from(byte));
        x.wrapping_sub(ONE_BYTES) & !x
    }
    let mut rv = chars.wrapping_sub(SPACES) & !chars;
    rv |= contains(chars, b'"') | contains(chars, b'\\');
    if HTML {
        rv |= contains(chars, b'<')
            | contains(chars, b'>')
            | contains(chars, b'&')
            | contains(chars, b'\'');
    }
    rv & (ONE_BYTES << 7)
}

#[inline(always)]
fn load_u64(input: &[u8], pos: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[pos..pos + 8]);
    u64::from_le_bytes(bytes)
}

/// Returns the index of the first byte at or after `pos` which needs
/// escaping or the length of the input if there is none.
///
/// The input is checked a word at a time, the tail of inputs with at least
/// a word is handled with an overlapping load of the last word.
#[inline]
fn find_escape<const HTML: bool>(input: &[u8], mut pos: usize) -> usize {
    let len = input.len();
    while pos + 8 <= len {
        let masked = escape_mask::<HTML>(load_u64(input, pos));
        if masked != 0 {
            return pos + masked.trailing_zeros() as usize / 8;
        }
        pos += 8;
    }
    if pos < len && len >= 8 {
        // the bytes of the overlapping last word before `pos` are replaced
        // by spaces (which do not need escaping), so the lowest flagged byte
        // is exact.
        let before = (1u64 << ((pos + 8 - len) * 8)) - 1;
        let word = (load_u64(input, len - 8) & !before) | (SPACES & before);
        let masked = escape_mask::<HTML>(word);
        return if masked != 0 {
            len - 8 + masked.trailing_zeros() as usize / 8
        } else {
            len
        };
    }
    while pos < len && !needs_escape::<HTML>(input[pos]) {
        pos += 1;
    }
    pos
}

#[cfg(feature = "speedups")]
#[inline]
fn write_int<I: itoa::Integer>(out: &mut String, value: I) {
    out.push_str(itoa::Buffer::new().format(value));
}

#[cfg(not(feature = "speedups"))]
#[inline]
fn write_int<I: std::fmt::Display>(out: &mut String, value: I) {
    use std::fmt::Write;
    write!(out, "{value}").unwrap();
}

/// Writes a finite float with the shortest text that reads back as the same
/// value.
#[cfg(feature = "speedups")]
#[inline]
fn write_finite_f64(out: &mut String, value: f64) {
    out.push_str(zmij::Buffer::new().format_finite(value));
}

/// Writes a finite float with the shortest text that reads back as the same
/// value.
#[cfg(not(feature = "speedups"))]
#[inline]
fn write_finite_f64(out: &mut String, value: f64) {
    format_finite_f64(out, value);
}

/// Formats a finite float like `zmij::Buffer::format_finite`.
///
/// The text has the shortest digits that read back as the value, in
/// scientific notation only for very large and very small values and always
/// with a `.` or an exponent (`1.0`, `0.001`, `1e+16`, `1.5e-7`).
#[cfg(any(not(feature = "speedups"), test))]
fn format_finite_f64(out: &mut String, value: f64) {
    use std::fmt::Write;
    debug_assert!(value.is_finite());
    // the standard library formats the shortest digits that read back
    let mut scientific = StackText::new();
    write!(scientific, "{:e}", value.abs()).unwrap();
    let (mantissa, exp) = scientific.as_str().split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    // the digits without the point, at most 17
    let mut buffer = [0u8; 17];
    let mut len = 0;
    for &byte in mantissa.as_bytes() {
        if byte != b'.' {
            buffer[len] = byte;
            len += 1;
        }
    }
    let digits = &mut buffer[..len];
    // if the value is exactly between two candidates, the standard library
    // rounds up and zmij to the even one.  Only if both read back: the gap
    // to the next smaller value of a power of two is half as large.
    let last = digits[len - 1];
    if (last - b'0') % 2 == 1 && is_tie(value.abs(), ascii(digits), exp) {
        digits[len - 1] = last - 1;
        let even = format!("{}e{}", ascii(digits), exp - (len as i32 - 1));
        if even.parse::<f64>().ok() != Some(value.abs()) {
            digits[len - 1] = last;
        }
    }
    let digits = ascii(digits);
    let len = len as i32;
    // the value is digits * 10^k and 10^(kk - 1) <= value < 10^kk
    let kk = exp + 1;
    let k = kk - len;
    if value.is_sign_negative() {
        out.push('-');
    }
    if 0 <= k && kk <= 16 {
        // 1234e7 -> 12340000000.0
        out.push_str(digits);
        for _ in 0..k {
            out.push('0');
        }
        out.push_str(".0");
    } else if 0 < kk && kk <= 16 {
        // 1234e-2 -> 12.34
        out.push_str(&digits[..kk as usize]);
        out.push('.');
        out.push_str(&digits[kk as usize..]);
    } else if -5 < kk && kk <= 0 {
        // 1234e-6 -> 0.001234
        out.push_str("0.");
        for _ in kk..0 {
            out.push('0');
        }
        out.push_str(digits);
    } else {
        // 1e30, 1234e30 -> 1.234e+33
        out.push_str(&digits[..1]);
        if len > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        if kk > 1 {
            out.push('+');
        }
        write!(out, "{}", kk - 1).unwrap();
    }
}

#[cfg(any(not(feature = "speedups"), test))]
fn ascii(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}

/// Text formatted on the stack.
///
/// This holds the scientific notation of floats (at most 24 bytes, like
/// `2.2250738585072014e-308`).
#[cfg(any(not(feature = "speedups"), test))]
struct StackText {
    bytes: [u8; 32],
    len: usize,
}

#[cfg(any(not(feature = "speedups"), test))]
impl StackText {
    fn new() -> StackText {
        StackText {
            bytes: [0; 32],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        ascii(&self.bytes[..self.len])
    }
}

#[cfg(any(not(feature = "speedups"), test))]
impl std::fmt::Write for StackText {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let end = self.len + s.len();
        self.bytes
            .get_mut(self.len..end)
            .ok_or(std::fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// Returns `true` if the value is exactly between the digits and the digits
/// with the last one decremented (with the same exponent).
///
/// The value is positive, the digits `d` (`n` of them) stand for
/// `d * 10^(exp - n + 1)`.  With `q = exp - n` the middle is
/// `(2d - 1) * 5 * 10^q`, which is `(2d - 1) * 5^(q + 1) * 2^q`.  The value
/// is `m * 2^e` with an odd `m`.  As the factors before the powers of two
/// are odd on both sides, they are equal if the exponents of two are and
/// the rest is.
#[cfg(any(not(feature = "speedups"), test))]
fn is_tie(value: f64, digits: &str, exp: i32) -> bool {
    let bits = value.to_bits();
    let (mut m, mut e) = match (bits >> 52) as i32 & 0x7ff {
        0 => (bits & ((1 << 52) - 1), -1074),
        biased => (bits & ((1 << 52) - 1) | (1 << 52), biased - 1075),
    };
    let zeros = m.trailing_zeros();
    m >>= zeros;
    e += zeros as i32;

    let q = exp - digits.len() as i32;
    if e != q {
        return false;
    }
    // at most 17 digits
    let odd = u128::from(digits.parse::<u64>().unwrap()) * 2 - 1;
    let m = u128::from(m);
    // the power of five on the side of the middle
    let p = q + 1;
    let (small, large, power) = if p >= 0 {
        (m, odd, p as u32)
    } else {
        (odd, m, p.unsigned_abs())
    };
    // both are below 2^64, a larger product cannot be equal
    5u128.checked_pow(power).and_then(|x| x.checked_mul(large)) == Some(small)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fallback(value: f64) -> String {
        let mut rv = String::new();
        format_finite_f64(&mut rv, value);
        rv
    }

    fn float(value: f64) -> String {
        to_json(&Value::from(value), JsonStyle::Compact, false).unwrap()
    }

    #[test]
    fn test_floats() {
        for (value, expected) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (123.456, "123.456"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.5e16, "1.5e+16"),
            (1234.5e20, "1.2345e+23"),
            (1e-4, "0.0001"),
            (1e-5, "0.00001"),
            (1.5e-5, "0.000015"),
            (1e-6, "1e-6"),
            (1.25e-7, "1.25e-7"),
            (f64::MAX, "1.7976931348623157e+308"),
            (f64::MIN_POSITIVE, "2.2250738585072014e-308"),
            (5e-324, "5e-324"),
            // exactly between two shortest candidates, the even one is used
            (165793407361858.0 + 0.125, "165793407361858.12"),
            (-(1149636667324797.0 + 0.25), "-1149636667324797.2"),
        ] {
            assert_eq!(float(value), expected, "{value:e}");
            assert_eq!(fallback(value), expected, "{value:e}");
        }
        assert_eq!(float(f64::NAN), "null");
        assert_eq!(float(f64::INFINITY), "null");
        assert_eq!(float(f64::NEG_INFINITY), "null");
    }

    #[cfg(feature = "speedups")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_fallback_like_zmij() {
        fn check(value: f64) {
            assert_eq!(
                fallback(value),
                zmij::Buffer::new().format_finite(value),
                "{value:e}"
            );
        }
        // the candidate below a power of two does not always read back
        for exp in -1074..1024 {
            check(2f64.powi(exp));
            check(-2f64.powi(exp));
        }
        let mut state = 0x2545f4914f6cdd1du64;
        for idx in 0..200_000u64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let value = if idx % 2 == 0 {
                f64::from_bits(state)
            } else {
                (state % 100_000_000) as f64 / 10f64.powi((state % 40) as i32 - 15)
            };
            if value.is_finite() {
                check(value);
            }
        }
    }

    #[test]
    fn test_ints() {
        let json = |value: Value| to_json(&value, JsonStyle::Compact, false).unwrap();
        assert_eq!(json(Value::from(0u64)), "0");
        assert_eq!(json(Value::from(u64::MAX)), "18446744073709551615");
        assert_eq!(json(Value::from(i64::MIN)), "-9223372036854775808");
        assert_eq!(
            json(Value::from(u128::MAX)),
            "340282366920938463463374607431768211455"
        );
        assert_eq!(
            json(Value::from(i128::MIN)),
            "-170141183460469231731687303715884105728"
        );
    }

    #[test]
    fn test_strings() {
        let json = |value: &str, html_safe| {
            to_json(&Value::from(value), JsonStyle::Compact, html_safe).unwrap()
        };
        assert_eq!(
            json("a\"b\\c\n\t\u{1}\u{7f}\u{fc}/<>&'", false),
            "\"a\\\"b\\\\c\\n\\t\\u0001\u{7f}\u{fc}/<>&'\""
        );
        assert_eq!(
            json("a\"b\\c\n\t\u{1}\u{7f}\u{fc}/<>&'", true),
            "\"a\\\"b\\\\c\\n\\t\\u0001\u{7f}\u{fc}/\\u003c\\u003e\\u0026\\u0027\""
        );
    }

    #[test]
    fn test_find_escape() {
        fn naive<const HTML: bool>(input: &[u8], pos: usize) -> usize {
            input[pos..]
                .iter()
                .position(|&c| needs_escape::<HTML>(c))
                .map_or(input.len(), |idx| pos + idx)
        }

        let alphabet: &[u8] = b"a\"\\\x00\x1f\x20\x7f\x80\xff\xe3<>&'";
        let mut state = 0x2545f4914f6cdd1du64;
        for len in 0..80 {
            for _ in 0..200 {
                let input: Vec<u8> = (0..len)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        if state % 16 == 0 {
                            alphabet[(state >> 8) as usize % alphabet.len()]
                        } else {
                            b'x'
                        }
                    })
                    .collect();
                for pos in 0..=len {
                    assert_eq!(
                        find_escape::<false>(&input, pos),
                        naive::<false>(&input, pos),
                        "{input:?} {pos}"
                    );
                    assert_eq!(
                        find_escape::<true>(&input, pos),
                        naive::<true>(&input, pos),
                        "{input:?} {pos}"
                    );
                }
            }
        }
    }
}
