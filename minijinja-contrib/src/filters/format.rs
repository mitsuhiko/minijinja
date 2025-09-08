use minijinja::{Error, ErrorKind, Value};

/// Formats a value according to the given spec.
///
/// The format spec passed as an argument to the filter describes _how_ the value
/// should be formatted: how much padding, what kind of alignment, to what precision,
/// in which numeric radix, and so on.
///
/// ```jinja
/// {{ 42 | fmt('04') }} -> "0042"
/// {{ 127 | fmt('#x') }} -> "0x7f"
/// {{ "Hello" | fmt('=^9') }} -> "==Hello=="
///
/// Weight is: {{ weight | fmt('.2f') }}kg -> Weight is: 42.00kg
/// ```
///
/// # Syntax
///
/// The formatting spec's syntax follows the grammar below:
///
/// ```text
/// format_spec: [[fill]align]['+']['#']['0'][width]['.'precision][type]
/// fill: character
/// align: '<' | '>' | '^'
/// width: digit+
/// precision: digit+
/// type: 'b' | 'e' | 'E' | 'f' | 'F' | 'o' | 'x' | 'X'
/// ```
///
/// # Width
///
/// This parameter specifies the minimum width the formatted value should take up. If
/// the value's text does not fill up the space, the remaining space gets filled up
/// according to the specified fill and alignment options.
///
/// ```jinja
/// {{ "Hello" | fmt("8") }} -> "Hello   "
/// {{ 123 | fmt("6") }} -> "   123"
/// ```
///
/// # Fill and align
///
/// An optional fill character and alignment can be specified in conjunction with the
/// `width` parameter, specifying how the text should be aligned and which character
/// should fill up the remaining space if needed.
///
/// The fill character can be any Unicode character. Each character is naively
/// assumed to occupy one column when printed, disregarding messy realities of
/// Unicode.
///
/// The space character is used as default when the fill character is not
/// provided. In case of integer values, the `0` flag can implicitly set the fill
/// character to be `0` (see below).
///
/// The alignment options are as below:
///
/// | Option | Meaning |
/// | ------ | ------- |
/// | `<`    | left-aligned within the available space (default for most values) |
/// | `>`    | right-aligned within the available space (default for numbers) |
/// | `^`    | centered within the available space |
///
/// Note that fill and align parameters are meaningless if minimum `width` is not
/// specified, and so are ignored if `width` is missing.
///
/// ```jinja
/// {{ "Hello" | fmt("=>8") }} -> "===Hello"
/// {{ "Hi" | fmt("=^4") }} -> "=Hi="
/// {{ "Hi" | fmt("👋<4") }} -> "Hi👋👋"
/// {{ 123 | fmt("=<6") }} -> "123==="
/// ```
///
/// # Sign
///
/// For negative numbers, the `-` sign is always printed. This is true even if the
/// number is printed in binary, octal, or hex format as the number does _not_ get
/// printed in two's complement form (see below).
///
/// For positive integers, the sign is omitted by default. The sign can be forced by
/// specifying the `+` flag.
///
/// ```jinja
/// {{ 123 | fmt("") }} -> "123"
/// {{ 123 | fmt("+") }} -> "+123"
/// {{ -123 | fmt("") }} -> "-123"
/// {{ -123 | fmt("+") }} -> "-123"
/// ```
///
/// # Alternate form (`#`)
///
/// The `#` flag specifies that an alternate form should be used while printing the
/// value. This option is applicable only for integers when a binary, octal, or
/// hexadecimal radix is used. This option adds '0b', '0o', '0x', or '0X' prefix
/// respectively to the output value.
///
/// The prefix is added after the sign and before the zero-padding, if any.
///
/// ```jinja
/// {{ 127 | fmt("x") }} -> "7f"
/// {{ 127 | fmt("#x") }} -> "0x7f"
/// {{ 127 | fmt("#06x") }} -> "0x007f"
/// ```
///
/// # Zero padding (`0`)
///
/// If the `width` is preceded by `'0'`, it enables sign-aware zero-padding for the
/// numbers. The formatted number is prefixed with zeros to fill the remaining space,
/// while also accounting for the sign character, if any. The padding zeros are
/// placed after the sign.
///
/// Note that this option overrides fill and align parameters.
///
/// # Precision
///
/// The precision is an integer indicating how many digits should get printed after
/// the decimal point for floating-point numbers, or for presentation types `'f'` (or
/// `'F'`) and `'e'` (or `'E'`).
///
/// For string values, the precision is considered as a "maximum width". If the
/// resulting string length is larger than this number, then it's truncated down to
/// these many characters first, and then gets printed according to the fill, align,
/// and width specification, if provided.
///
/// ```jinja
/// {{ pi | fmt(".2") }} -> "3.14"
/// {{ pi | fmt("08.4f") }} -> "003.1416"
/// {{ "Hello" | fmt("4.2") }} -> "He  "
/// ```
///
/// # Type
///
/// The type specifies how to convert the value into string format before applying
/// other formatting options.
///
/// The following types are supported:
///
/// | Type | Meaning |
/// | ---- | ------- |
/// | `b`  | Binary format. Prints integer in base 2. |
/// | `e`  | Scientific notation. Prints integer of floating point number in scientific notation with significand and exponent separated by lower-case `e`.  Significand is printed as per the specified `precision`, or using `6` digits precision by default. |
/// | `E`  | Scientific notation. Same as `F`, but uses upper-case `E` as a separator. |
/// | `f`  | Fixed-point notation. Prints integer or floating point number in decimal with exactly `precision` digits following the decimal point. Uses `6` digits if `precision` is not provided in the spec. |
/// | `F`  | Fixed-point notation. Same as `f`, but prints `nan` as `NAN`, and `inf` as `INF`. |
/// | `o`  | Octal format. Prints integer in base 8. |
/// | `x`  | Hexadecimal format. Prints integer in base 16 using lowercase alphabet. |
/// | `X`  | Hexadecimal format. Prints integer in base 16 using uppercase alphabet. |
///
/// Note that for `b`, `o`, and `x` (or `X`) formats, a negative number is printed as
/// `-` sign followed by the number's absolute value in the specified radix (and
/// zero-padding and `#`-format prefix if any). This is different from number's
/// internal two's complement representation, which requires the integer's width
/// (e.g. 32 or 64bits) to be known. This behavior is adopted from Python.
pub fn fmt(input: &Value, format_spec: String) -> Result<String, Error> {
    let spec = parse_spec(&format_spec)?;
    spec.format(input)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Align {
    Left,
    Right,
    Center,
}

#[derive(Debug, PartialEq, Eq)]
struct FillAlign {
    fill: Option<char>,
    align: Align,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Type {
    Default,
    Binary,
    LowerE,
    UpperE,
    LowerF,
    UpperF,
    Octal,
    LowerHex,
    UpperHex,
}

impl Type {
    fn description(&self) -> &'static str {
        match self {
            Type::Default => "",
            Type::Binary => "binary format ('b')",
            Type::LowerE => "scientific notation ('e')",
            Type::UpperE => "scientific notation ('E')",
            Type::LowerF => "fixed-point notation ('f')",
            Type::UpperF => "fixed-point notation ('F')",
            Type::Octal => "octal format ('o')",
            Type::LowerHex => "hex format ('x')",
            Type::UpperHex => "hex format ('X')",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FormatSpec {
    fill_align: Option<FillAlign>,
    print_sign: bool,
    alternate_form: bool,
    zero_padded: bool,
    width: Option<usize>,
    precision: Option<usize>,
    typ: Type,
}

impl FormatSpec {
    fn format(&self, val: &Value) -> Result<String, Error> {
        if let Ok(boolean) = bool::try_from(val.clone()) {
            Ok(self.format_bool(boolean))
        } else if let Some((number, is_negative)) = Self::cast_to_abs_integer(val) {
            Ok(self.format_integer(number, is_negative))
        } else if let Ok(fp) = f64::try_from(val.clone()) {
            self.format_float(fp)
        } else {
            if Type::Default != self.typ {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!(
                        "{} cannot be formatted in {}",
                        val.kind(),
                        self.typ.description()
                    ),
                ));
            }

            Ok(self.format_str(val.to_string()))
        }
    }

    fn cast_to_abs_integer(val: &Value) -> Option<(u128, bool)> {
        if !val.is_integer() {
            return None;
        }

        if let Ok(i) = i128::try_from(val.clone()) {
            Some((i.unsigned_abs(), i.is_negative()))
        } else if let Ok(u) = u128::try_from(val.clone()) {
            Some((u, false))
        } else {
            None
        }
    }

    fn format_bool(&self, val: bool) -> String {
        if let Type::Default = self.typ {
            // format "true" or "false" as a regular string, but without truncating
            self.apply_padding(format!("{val}"), Align::Left)
        } else {
            self.format_integer(if val { 1 } else { 0 }, false)
        }
    }

    fn format_str(&self, text: String) -> String {
        if let Some(p) = &self.precision {
            if *p < text.len() {
                return self.apply_padding(text[..*p].to_string(), Align::Left);
            }
        }

        self.apply_padding(text, Align::Left)
    }

    fn format_integer(&self, val: u128, is_negative: bool) -> String {
        let sign = if is_negative {
            "-"
        } else if self.print_sign {
            "+"
        } else {
            ""
        };

        let number = match &self.typ {
            Type::Binary => format!("{val:b}"),
            Type::Octal => format!("{val:o}"),
            Type::LowerHex => format!("{val:x}"),
            Type::UpperHex => format!("{val:X}"),
            Type::Default => format!("{val}"),
            Type::LowerE => {
                if let Some(p) = &self.precision {
                    format!("{:.p$e}", val as f64)
                } else {
                    format!("{:.6e}", val as f64)
                }
            }
            Type::UpperE => {
                if let Some(p) = &self.precision {
                    format!("{:.p$E}", val as f64)
                } else {
                    format!("{:.6E}", val as f64)
                }
            }
            Type::LowerF | Type::UpperF => {
                if let Some(p) = &self.precision {
                    format!("{:.p$}", val as f64)
                } else {
                    format!("{:.6}", val as f64)
                }
            }
        };

        self.format_number(&number, sign)
    }

    fn format_float(&self, val: f64) -> Result<String, Error> {
        let sign = if val.is_sign_negative() {
            "-"
        } else if self.print_sign {
            "+"
        } else {
            ""
        };

        match &self.typ {
            Type::Default => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("NaN", ""))
                } else if val.is_infinite() {
                    Ok(self.format_number("inf", sign))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$}", val.abs())
                    } else {
                        let mut fl_num = format!("{}", val.abs());
                        if !fl_num.contains('.') {
                            fl_num.push_str(".0");
                        }
                        fl_num
                    };
                    Ok(self.format_number(&num, sign))
                }
            }
            Type::LowerE => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("NaN", ""))
                } else if val.is_infinite() {
                    Ok(self.format_number("inf", sign))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$e}", val.abs())
                    } else {
                        format!("{:.6e}", val.abs())
                    };
                    Ok(self.format_number(&num, sign))
                }
            }
            Type::UpperE => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("NaN", ""))
                } else if val.is_infinite() {
                    Ok(self.format_number("inf", sign))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$E}", val.abs())
                    } else {
                        format!("{:.6E}", val.abs())
                    };
                    Ok(self.format_number(&num, sign))
                }
            }
            Type::LowerF => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("nan", ""))
                } else if val.is_infinite() {
                    Ok(self.format_number("inf", sign))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$}", val.abs())
                    } else {
                        format!("{:.6}", val.abs())
                    };
                    Ok(self.format_number(&num, sign))
                }
            }
            Type::UpperF => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("NAN", ""))
                } else if val.is_infinite() {
                    Ok(self.format_number("INF", sign))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$}", val.abs())
                    } else {
                        format!("{:.6}", val.abs())
                    };
                    Ok(self.format_number(&num, sign))
                }
            }
            Type::Binary | Type::Octal | Type::LowerHex | Type::UpperHex => Err(Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "'float' value cannot be formatted in {}",
                    self.typ.description()
                ),
            )),
        }
    }

    fn format_number(&self, number: &str, sign: &str) -> String {
        let radix = if self.alternate_form {
            match &self.typ {
                Type::Default | Type::LowerE | Type::UpperE | Type::LowerF | Type::UpperF => "",
                Type::Binary => "0b",
                Type::Octal => "0o",
                Type::LowerHex | Type::UpperHex => "0x",
            }
        } else {
            ""
        };

        if self.zero_padded {
            let min_width = self
                .width
                .expect("zero-padding must have been parsed along with width");
            let curr_width = sign.len() + radix.len() + number.len();
            if curr_width < min_width {
                let fill_width = min_width - curr_width;
                let filler = "0".repeat(fill_width);
                format!("{sign}{radix}{filler}{number}")
            } else {
                format!("{sign}{radix}{number}")
            }
        } else {
            let unpadded = format!("{sign}{radix}{number}");
            self.apply_padding(unpadded, Align::Right)
        }
    }

    fn apply_padding(&self, text: String, default_align: Align) -> String {
        let curr_width = text.len();
        if let Some(min_width) = &self.width {
            if curr_width < *min_width {
                let fill_width = min_width - curr_width;
                let (fill_char, align) = match &self.fill_align {
                    Some(FillAlign { fill: None, align }) => (' ', *align),
                    Some(FillAlign {
                        fill: Some(f),
                        align,
                    }) => (*f, *align),
                    None => (' ', default_align),
                };
                let res = match align {
                    Align::Left => {
                        let filler = String::from(fill_char).repeat(fill_width);
                        format!("{text}{filler}")
                    }
                    Align::Right => {
                        let filler = String::from(fill_char).repeat(fill_width);
                        format!("{filler}{text}")
                    }
                    Align::Center => {
                        let left_width = fill_width / 2;
                        let right_width = fill_width - left_width;
                        let fill = String::from(fill_char);
                        let left_filler = fill.repeat(left_width);
                        let right_filler = fill.repeat(right_width);
                        format!("{left_filler}{text}{right_filler}")
                    }
                };
                return res;
            }
        }
        text
    }
}

fn parse_spec(input: &str) -> Result<FormatSpec, Error> {
    let (remaining, fill_align) = parse_fill_align(input);
    let (remaining, print_sign) = parse_flag(remaining, '+');
    let (remaining, alternate_form) = parse_flag(remaining, '#');
    let (remaining, mut zero_padded) = parse_flag(remaining, '0');
    let (remaining, mut width) = parse_integer(remaining)?;
    if zero_padded && width.is_none() {
        // if '0' is not followed by width (i.e. digit+), then it should be parsed as
        // a width, not as zero-padding.
        zero_padded = false;
        width = Some(0);
    }
    let (remaining, precision) = parse_precision(remaining)?;
    let (remaining, typ) = parse_type(remaining);

    if !remaining.is_empty() {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "invalid character sequence '{}' in the format spec",
                remaining
            ),
        ))
    } else {
        Ok(FormatSpec {
            fill_align,
            print_sign,
            alternate_form,
            zero_padded,
            width,
            precision,
            typ,
        })
    }
}

fn parse_fill_align(input: &str) -> (&str, Option<FillAlign>) {
    let maybe_fill = input.chars().next();
    let maybe_align = input.chars().nth(1);

    let (consumed, fa) = match (maybe_fill, maybe_align) {
        (Some(f), Some('<')) => (
            f.len_utf8() + 1,
            FillAlign {
                fill: Some(f),
                align: Align::Left,
            },
        ),
        (Some(f), Some('>')) => (
            f.len_utf8() + 1,
            FillAlign {
                fill: Some(f),
                align: Align::Right,
            },
        ),
        (Some(f), Some('^')) => (
            f.len_utf8() + 1,
            FillAlign {
                fill: Some(f),
                align: Align::Center,
            },
        ),
        (Some('<'), _) => (
            1,
            FillAlign {
                fill: None,
                align: Align::Left,
            },
        ),
        (Some('>'), _) => (
            1,
            FillAlign {
                fill: None,
                align: Align::Right,
            },
        ),
        (Some('^'), _) => (
            1,
            FillAlign {
                fill: None,
                align: Align::Center,
            },
        ),
        (_, _) => return (input, None),
    };

    (&input[consumed..], Some(fa))
}

fn parse_flag(input: &str, f: char) -> (&str, bool) {
    if input.starts_with(f) {
        (&input[f.len_utf8()..], true)
    } else {
        (input, false)
    }
}

fn parse_integer(input: &str) -> Result<(&str, Option<usize>), Error> {
    let digit_count = input.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 {
        Ok((input, None))
    } else {
        let num = input[0..digit_count].parse::<usize>().map_err(|e| {
            Error::new(
                ErrorKind::InvalidOperation,
                "invalid integer in the format spec",
            )
            .with_source(e)
        })?;
        Ok((&input[digit_count..], Some(num)))
    }
}

fn parse_precision(input: &str) -> Result<(&str, Option<usize>), Error> {
    let (remaining, has_precision) = parse_flag(input, '.');
    if !has_precision {
        return Ok((remaining, None));
    }

    let (remaining, maybe_precision) = parse_integer(remaining)?;
    if let Some(precision) = maybe_precision {
        Ok((remaining, Some(precision)))
    } else {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "expecting an integer after '.' in the format spec",
        ))
    }
}

fn parse_type(input: &str) -> (&str, Type) {
    let typ = match input.chars().next() {
        Some('b') => Type::Binary,
        Some('e') => Type::LowerE,
        Some('E') => Type::UpperE,
        Some('f') => Type::LowerF,
        Some('F') => Type::UpperF,
        Some('o') => Type::Octal,
        Some('x') => Type::LowerHex,
        Some('X') => Type::UpperHex,
        _ => return (input, Type::Default),
    };
    (&input[1..], typ)
}

#[cfg(test)]
mod tests {
    use super::*;
    use similar_asserts::assert_eq;

    #[test]
    fn test_spec_parser() {
        fn parse(input: &str) -> FormatSpec {
            parse_spec(input).expect("must be a valid spec")
        }

        let spec = parse("x<10");
        assert_eq!(
            spec,
            FormatSpec {
                fill_align: Some(FillAlign {
                    fill: Some('x'),
                    align: Align::Left
                }),
                print_sign: false,
                alternate_form: false,
                zero_padded: false,
                width: Some(10),
                precision: None,
                typ: Type::Default,
            }
        );

        fn read_fill_align(input: &str) -> (Option<char>, Align) {
            let spec = parse(input);
            let fa = spec.fill_align.unwrap();
            (fa.fill, fa.align)
        }

        assert_eq!(read_fill_align("<<10"), (Some('<'), Align::Left));
        assert_eq!(read_fill_align("^10"), (None, Align::Center));
        assert_eq!(read_fill_align("+>10"), (Some('+'), Align::Right));

        let spec = parse("+010x");
        assert_eq!(
            spec,
            FormatSpec {
                fill_align: None,
                print_sign: true,
                alternate_form: false,
                zero_padded: true,
                width: Some(10),
                precision: None,
                typ: Type::LowerHex
            }
        );

        let spec = parse("0010");
        assert!(spec.zero_padded);
        assert_eq!(spec.width, Some(10));

        let spec = parse("#10X");
        assert_eq!(
            spec,
            FormatSpec {
                fill_align: None,
                print_sign: false,
                alternate_form: true,
                zero_padded: false,
                width: Some(10),
                precision: None,
                typ: Type::UpperHex
            }
        );

        let spec = parse("4.2f");
        assert_eq!(
            spec,
            FormatSpec {
                fill_align: None,
                print_sign: false,
                alternate_form: false,
                zero_padded: false,
                width: Some(4),
                precision: Some(2),
                typ: Type::LowerF
            }
        );
    }
}
