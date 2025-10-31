use crate::value::ValueKind;
use crate::{Error, ErrorKind, Value};

/// This enum indicates the style of the format string passed to the
/// [`format_filter`] function.
///
/// Like jinja2, minijinja also supports two flavours of string formatting:
/// - printf-style: `{{ "%s, %s!"|format(greeting, name) }}`
/// - str.format style: `{{ "{}, {}!".format(greeting, name) }}` (through `minijinja-contrib`'s `pycompat` feature)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FormatStyle {
    /// Printf-style format string as described
    /// [here](https://docs.python.org/3/library/stdtypes.html#printf-style-string-formatting)
    Printf,
    /// `str.format` style format string, described
    /// [here](https://docs.python.org/3/library/string.html#format-string-syntax)
    StrFormat,
}

/// A helper function to apply a set of values to a given format string.  The format
/// string could be in one of the two styles specified by the [`FormatStyle`] enum.
pub fn format_filter(
    style: FormatStyle,
    format_str: &str,
    args: &[Value],
) -> Result<String, Error> {
    match style {
        FormatStyle::Printf => printf_style::format(format_str, args),
        FormatStyle::StrFormat => str_format_style::format(format_str, args),
    }
}

// Token produced by the format string parser
#[derive(Debug)]
enum Token<'src> {
    // Text that must be copied verbatim
    Literal(&'src str),
    // Field that must be replaced with formatted text
    Replace(ReplacementField<'src>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PathElem<'src> {
    Attr(&'src str),
    Key(&'src str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FieldName<'src> {
    Kwarg(&'src str, Vec<PathElem<'src>>),
    Positional(usize, Vec<PathElem<'src>>),
    MappingKey(&'src str),
}

#[derive(Debug, PartialEq, Eq)]
struct ReplacementField<'src> {
    field_name: Option<FieldName<'src>>,
    format_spec: Option<FormatSpec>,
    location: usize,
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
    Decimal,
    Octal,
    LowerHex,
    UpperHex,
    LowerE,
    UpperE,
    LowerF,
    UpperF,
    String,
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
            Type::Decimal => "decimal format ('d')",
            Type::String => "string format ('s')",
        }
    }
}

// Captures format spec for both printf-style and str.format style format strings.
#[derive(Debug, PartialEq, Eq)]
struct FormatSpec {
    fill_align: Option<FillAlign>,
    print_sign: bool,
    space_before_positive_num: bool,
    alternate_form: bool,
    zero_padded: bool,
    width: Option<usize>,
    precision: Option<usize>,
    ty: Type,

    // Whether this spec is parsed from a printf-style format string
    format_style: FormatStyle,
    location: usize,
}

impl FormatSpec {
    // Format the given value according to this spec
    fn format(&self, val: &Value) -> Result<String, Error> {
        if let Ok(boolean) = bool::try_from(val.clone()) {
            self.format_bool(boolean)
        } else if let Some((number, is_negative)) = Self::cast_to_abs_integer(val) {
            self.format_integer(number, is_negative)
        } else if let Ok(fp) = f64::try_from(val.clone()) {
            self.format_float(fp)
        } else {
            self.format_str(val.to_string())
        }
    }

    fn type_conversion_err(&self, val_kind: &str, ty: Type) -> Error {
        Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "invalid format spec at offset {}; '{}' cannot be formatted in {}",
                self.location,
                val_kind,
                ty.description()
            ),
        )
    }

    // Returns absolute value of the integer and a boolean indicating if it's a
    // negative integer, if the Value is an integer; otherwise returns None.
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

    fn format_bool(&self, val: bool) -> Result<String, Error> {
        let treat_as_integer = self.fill_align.is_some()
            || self.print_sign
            || self.alternate_form
            || self.zero_padded
            || self.width.is_some()
            || self.precision.is_some();

        match self.ty {
            Type::Default if !treat_as_integer => {
                // Format "true" or "false" as a regular string, ignoring the
                // precision (i.e. without truncating)
                Ok(self.apply_padding(format!("{val}"), Align::Left))
            }
            Type::String => match self.format_style {
                FormatStyle::Printf => Ok(self.apply_padding(format!("{val}"), Align::Right)),
                FormatStyle::StrFormat => Err(self.type_conversion_err("bool", Type::String)),
            },
            Type::Default
            | Type::Binary
            | Type::Decimal
            | Type::Octal
            | Type::LowerHex
            | Type::UpperHex
            | Type::LowerE
            | Type::UpperE
            | Type::LowerF
            | Type::UpperF => self.format_integer(if val { 1 } else { 0 }, false),
        }
    }

    fn format_str(&self, text: String) -> Result<String, Error> {
        match self.ty {
            Type::Default | Type::String => {
                let default_align = match self.format_style {
                    FormatStyle::Printf => Align::Right,
                    FormatStyle::StrFormat => Align::Left,
                };

                if let Some(p) = &self.precision {
                    if *p < text.len() {
                        return Ok(self.apply_padding(text[..*p].to_string(), default_align));
                    }
                }
                Ok(self.apply_padding(text, default_align))
            }
            Type::Binary
            | Type::Decimal
            | Type::Octal
            | Type::LowerHex
            | Type::UpperHex
            | Type::LowerE
            | Type::UpperE
            | Type::LowerF
            | Type::UpperF => Err(self.type_conversion_err("string", self.ty)),
        }
    }

    fn format_integer(&self, val: u128, is_negative: bool) -> Result<String, Error> {
        let mut sign = if is_negative {
            "-"
        } else if self.print_sign {
            "+"
        } else if self.space_before_positive_num {
            " "
        } else {
            ""
        };

        let number = match self.ty {
            Type::Binary => format!("{val:b}"),
            Type::Octal => format!("{val:o}"),
            Type::LowerHex => format!("{val:x}"),
            Type::UpperHex => format!("{val:X}"),
            Type::Default | Type::Decimal => format!("{val}"),
            Type::String => {
                if let FormatStyle::Printf = self.format_style {
                    // printf-style formatting in Python ignores sign character flag
                    // '+' when combined with 's' format.
                    sign = if is_negative { "-" } else { "" };
                    format!("{val}")
                } else {
                    return Err(self.type_conversion_err("integer", Type::String));
                }
            }
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

        Ok(self.format_number(number, sign, false))
    }

    fn format_float(&self, val: f64) -> Result<String, Error> {
        let sign = if val.is_sign_negative() {
            "-"
        } else if self.print_sign && self.ty != Type::String {
            "+"
        } else if val.is_sign_positive() && self.space_before_positive_num {
            " "
        } else {
            ""
        };

        match self.ty {
            Type::String if FormatStyle::Printf != self.format_style => {
                Err(self.type_conversion_err("float", Type::String))
            }
            Type::Default | Type::String => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("nan".to_string(), "", true))
                } else if val.is_infinite() {
                    Ok(self.format_number("inf".to_string(), sign, true))
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
                    Ok(self.format_number(num, sign, false))
                }
            }
            Type::LowerE => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("nan".to_string(), "", true))
                } else if val.is_infinite() {
                    Ok(self.format_number("inf".to_string(), sign, true))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$e}", val.abs())
                    } else {
                        format!("{:.6e}", val.abs())
                    };
                    Ok(self.format_number(num, sign, false))
                }
            }
            Type::UpperE => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("NAN".to_string(), "", true))
                } else if val.is_infinite() {
                    Ok(self.format_number("INF".to_string(), sign, true))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$E}", val.abs())
                    } else {
                        format!("{:.6E}", val.abs())
                    };
                    Ok(self.format_number(num, sign, false))
                }
            }
            Type::LowerF => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("nan".to_string(), "", true))
                } else if val.is_infinite() {
                    Ok(self.format_number("inf".to_string(), sign, true))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$}", val.abs())
                    } else {
                        format!("{:.6}", val.abs())
                    };
                    Ok(self.format_number(num, sign, false))
                }
            }
            Type::UpperF => {
                if val.is_nan() {
                    // Sign has no meaning for NaN, so never print it
                    Ok(self.format_number("NAN".to_string(), "", true))
                } else if val.is_infinite() {
                    Ok(self.format_number("INF".to_string(), sign, true))
                } else {
                    let num = if let Some(p) = &self.precision {
                        format!("{:.p$}", val.abs())
                    } else {
                        format!("{:.6}", val.abs())
                    };
                    Ok(self.format_number(num, sign, false))
                }
            }
            Type::Binary | Type::Octal | Type::LowerHex | Type::UpperHex | Type::Decimal => {
                Err(self.type_conversion_err("float", self.ty))
            }
        }
    }

    fn format_number(&self, mut number: String, sign: &str, nan_or_inf: bool) -> String {
        let mut radix = "";

        // process alternate form flag `#`
        if self.alternate_form {
            match self.ty {
                Type::Default | Type::String | Type::Decimal => {}

                Type::LowerE | Type::UpperE | Type::LowerF | Type::UpperF => match self.precision {
                    Some(0) if !nan_or_inf => {
                        // Python inserts trailing '.' if precision is zero and alternate form is used
                        let coeff_end = number
                            .as_bytes()
                            .iter()
                            .take_while(|c| !matches!(c, b'e' | b'E'))
                            .count();
                        number.insert(coeff_end, '.');
                    }
                    _ => {}
                },

                Type::Binary => radix = "0b",
                Type::Octal => radix = "0o",
                Type::LowerHex => radix = "0x",
                Type::UpperHex => radix = "0X",
            }
        }

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

// Cursor over the input format string, providing helper functions to parser
// implementations of two different format styles.
struct Cursor<'s> {
    source: &'s str,
    current_offset: usize,
}

impl<'s> Cursor<'s> {
    fn new(source: &'s str) -> Self {
        Self {
            source,
            current_offset: 0,
        }
    }

    #[inline]
    fn rest(&self) -> &'s str {
        &self.source[self.current_offset..]
    }

    #[inline]
    fn rest_bytes(&self) -> &'s [u8] {
        &self.source.as_bytes()[self.current_offset..]
    }

    fn advance(&mut self, bytes: usize) -> &'s str {
        let consumed = &self.rest()[..bytes];
        self.current_offset += bytes;
        consumed
    }

    fn advance_if(&mut self, ascii_char: u8) -> bool {
        match self.rest_bytes().get(0) {
            Some(next) if *next == ascii_char => {
                self.advance(1);
                true
            }
            _ => false,
        }
    }

    #[inline]
    fn is_end(&self) -> bool {
        self.source.len() == self.current_offset
    }

    #[inline]
    fn position(&self) -> usize {
        self.current_offset
    }

    #[inline]
    fn source(&self) -> &'s str {
        self.source
    }
}

// Top-level tokenizer producing `Token`s out of the input format string. It invokes
// the style-specific parser to produce the `Replace` token when a replacement field
// is encountered.
struct Tokenizer<'s> {
    cursor: Cursor<'s>,
    format_style: FormatStyle,
}

impl<'s> Tokenizer<'s> {
    fn new(source: &'s str, format_style: FormatStyle) -> Self {
        Self {
            cursor: Cursor::new(source),
            format_style,
        }
    }

    fn next_token(&mut self) -> Result<Option<Token<'s>>, Error> {
        let mut offset = 0;
        let mut found_spec = false;
        let bytes = self.cursor.rest_bytes();
        let delimiter = match self.format_style {
            FormatStyle::Printf => b'%',
            FormatStyle::StrFormat => b'{',
        };

        loop {
            match bytes.get(offset) {
                Some(c) if *c == delimiter => {
                    // check for escape sequence
                    match bytes.get(offset + 1) {
                        Some(n) if *n == delimiter => {
                            // jump over escape sequence
                            offset += 2;
                        }
                        _ => {
                            // start of format spec, break without consuming the delimiter
                            found_spec = true;
                            break;
                        }
                    }
                }
                Some(_) => {
                    offset += 1;
                }
                None => break,
            }
        }
        if offset > 0 {
            let tok = Token::Literal(self.cursor.advance(offset));
            Ok(Some(tok))
        } else if found_spec {
            let field = match self.format_style {
                FormatStyle::Printf => printf_style::replacement_field(&mut self.cursor)?,
                FormatStyle::StrFormat => str_format_style::replacement_field(&mut self.cursor)?,
            };
            Ok(Some(Token::Replace(field)))
        } else {
            Ok(None)
        }
    }
}

fn parse_number(cursor: &mut Cursor) -> Result<Option<usize>, Error> {
    let digit_count = cursor
        .rest_bytes()
        .iter()
        .take_while(|c| c.is_ascii_digit())
        .count();
    if digit_count == 0 {
        Ok(None)
    } else {
        let num_str = cursor.advance(digit_count);
        let num = num_str.parse::<usize>().map_err(|e| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "invalid integer in the format string at offset {}",
                    cursor.position()
                ),
            )
            .with_source(e)
        })?;
        Ok(Some(num))
    }
}

fn parse_type(cursor: &mut Cursor, style: FormatStyle) -> Result<Type, Error> {
    let t = match cursor.rest_bytes().get(0) {
        Some(b'b') if FormatStyle::StrFormat == style => Type::Binary,
        Some(b'd') => Type::Decimal,
        Some(b'i') if FormatStyle::Printf == style => Type::Decimal,
        Some(b'e') => Type::LowerE,
        Some(b'E') => Type::UpperE,
        Some(b'f') => Type::LowerF,
        Some(b'F') => Type::UpperF,
        Some(b'o') => Type::Octal,
        Some(b'x') => Type::LowerHex,
        Some(b'X') => Type::UpperHex,
        Some(b's') => Type::String,
        Some(b'}') if FormatStyle::StrFormat == style => {
            // end of spec, return without consuming '}'
            return Ok(Type::Default);
        }
        Some(c) => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "invalid conversion type '{}' in format spec at offset {}",
                    *c as char,
                    cursor.position()
                ),
            ))
        }
        None => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "incomplete format spec at offset {}; missing conversion type",
                    cursor.position()
                ),
            ))
        }
    };
    cursor.advance(1);
    Ok(t)
}

fn parse_till<'s>(cursor: &mut Cursor<'s>, end_delim: u8) -> Result<&'s str, Error> {
    let start = cursor.position();
    loop {
        if cursor.advance_if(end_delim) {
            break;
        } else if cursor.is_end() {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "incomplete format key at offset {}; missing closing '{}'",
                    start, end_delim as char
                ),
            ));
        } else {
            cursor.advance(1);
        }
    }
    // don't include the closing delimiter
    let end = cursor.position() - 1;
    Ok(&cursor.source()[start..end])
}

mod printf_style {
    // module implementing prinf-style specific parser and formatter functions.

    use super::*;

    // Printf-style field parser parsing the following grammar:
    //
    // replacement_field -> '%' [key] format_spec
    // key -> '(' char* ')'
    // format_spec -> flag* [width] ['.' precision] [len_modifier] type
    // flag -> '#' | '0' | '-' | ' ' | '+'
    // width -> number | '*'
    // precision -> number | '*'
    // number -> [0-9]+
    // len_modifier -> 'h' | 'l' | 'L'
    // type -> 'd' | 'i' | 'o' | 'x' | 'X' | 'e' | 'E' | 'f' | 'F' | 's'
    pub fn replacement_field<'s>(cursor: &mut Cursor<'s>) -> Result<ReplacementField<'s>, Error> {
        let location = cursor.position();
        // consume '%'
        cursor.advance(1);

        let field_name = parse_key(cursor)?.map(FieldName::MappingKey);
        let spec = parse_format_spec(cursor)?;
        Ok(ReplacementField {
            field_name,
            format_spec: Some(spec),
            location,
        })
    }

    fn parse_key<'s>(cursor: &mut Cursor<'s>) -> Result<Option<&'s str>, Error> {
        if cursor.advance_if(b'(') {
            Ok(Some(parse_till(cursor, b')')?))
        } else {
            Ok(None)
        }
    }

    fn parse_format_spec(cursor: &mut Cursor) -> Result<FormatSpec, Error> {
        let location = cursor.position();
        let mut fill_align = None;
        let mut print_sign = false;
        let mut space_before_positive_num = false;
        let mut alternate_form = false;
        let mut zero_padded = false;

        loop {
            match cursor.rest_bytes().get(0) {
                Some(b'#') => alternate_form = true,
                Some(b'0') => zero_padded = true,
                Some(b'-') => {
                    fill_align = Some(FillAlign {
                        fill: None,
                        align: Align::Left,
                    })
                }
                Some(b' ') => space_before_positive_num = true,
                Some(b'+') => print_sign = true,
                _ => break,
            }
            cursor.advance(1);
        }

        if print_sign {
            // '+' flag overrides ' '
            space_before_positive_num = false;
        }

        if let Some(FillAlign {
            align: Align::Left, ..
        }) = fill_align
        {
            // '-' flag overrides '0' padding flag
            zero_padded = false;
        }

        let mut width = parse_number(cursor)?;
        if zero_padded && width.is_none() {
            // if '0' is not followed by width (i.e. digit+), then it should be parsed as
            // a width, not as zero-padding.
            zero_padded = false;
            width = Some(0);
        }

        let precision = cursor
            .advance_if(b'.')
            .then(|| parse_number(cursor))
            .transpose()?
            .flatten();

        // length modifier is ignored in Python
        parse_len_modifier(cursor);
        let ty = parse_type(cursor, FormatStyle::Printf)?;
        Ok(FormatSpec {
            fill_align,
            print_sign,
            space_before_positive_num,
            alternate_form,
            zero_padded,
            width,
            precision,
            ty,
            format_style: FormatStyle::Printf,
            location,
        })
    }

    fn parse_len_modifier(cursor: &mut Cursor) {
        match cursor.rest_bytes().get(0) {
            Some(b'h') | Some(b'l') | Some(b'L') => {
                cursor.advance(1);
            }
            _ => (),
        }
    }

    // Do prinf-style formatting. Parse the format string and apply values from args
    // to the fields found in the string, by formatting the value according to the
    // spec found in the field.
    pub fn format(format_str: &str, args: &[Value]) -> Result<String, Error> {
        let mut input = Tokenizer::new(format_str, FormatStyle::Printf);
        let mut result = String::new();
        let mut arg_index = 0;

        fn missing_arg_err(location: usize) -> Error {
            Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "missing an argument for format spec at offset '{}'",
                    location
                ),
            )
        }

        while let Some(token) = input.next_token()? {
            match token {
                Token::Literal(lit) => result.push_str(lit),
                Token::Replace(ReplacementField {
                    field_name,
                    format_spec,
                    ..
                }) => {
                    let spec = format_spec.expect("printf-style format must specify a spec");
                    let arg = {
                        if let Some(FieldName::MappingKey(key)) = field_name {
                            // only a mapping as an argument is expected, and the key must be
                            // read from the provided mapping.
                            if let Some(arg) = args.get(0) {
                                if arg.kind() != ValueKind::Map {
                                    return Err(Error::new(
                                        ErrorKind::InvalidOperation,
                                        "format argument must be a mapping",
                                    ));
                                }

                                match arg.get_attr(key).ok() {
                                    Some(val) if !val.is_undefined() => val,
                                    _ => return Err(missing_arg_err(spec.location)),
                                }
                            } else {
                                return Err(missing_arg_err(spec.location));
                            }
                        } else if let Some(arg) = args.get(arg_index) {
                            arg_index += 1;
                            arg.clone()
                        } else {
                            return Err(missing_arg_err(spec.location));
                        }
                    };
                    result.push_str(&spec.format(&arg)?);
                }
            }
        }
        Ok(result)
    }
}

mod str_format_style {
    // module implementing `str.format`-style specific parser and formatter
    // functions.

    use super::*;
    use crate::value::{from_args, Kwargs};

    // Field parser parsing the following grammar:
    //
    // replacement_field -> '{' [field_name] [':' format_spec] '}'
    // field_name -> arg_name path
    // arg_name -> identifier | number
    // path -> ('.' identifier | '[' elem_index ']')*
    // elem_index -> number | char*
    // format_spec -> [options] [width] ['.' precision] [type]
    // options -> [[fill] align] [sign] ['#'] ['0']
    // fill -> char
    // align -> '<' | '>' | '^'
    // sign -> '+' | '-' | ' '
    // width -> number
    // precision -> number
    // number -> [0-9]+
    // type -> 'b' | 'd' | 'o' | 'x' | 'X' | 'e' | 'E' | 'f' | 'F' | 's'
    pub fn replacement_field<'s>(cursor: &mut Cursor<'s>) -> Result<ReplacementField<'s>, Error> {
        let location = cursor.position();
        // consume '{'
        cursor.advance(1);
        let field_name = parse_field_name(cursor)?;
        let format_spec = if cursor.advance_if(b':') {
            Some(parse_format_spec(cursor)?)
        } else {
            None
        };

        if cursor.advance_if(b'}') {
            Ok(ReplacementField {
                field_name,
                format_spec,
                location,
            })
        } else {
            let err = if let Some(&n) = cursor.rest_bytes().get(0) {
                format!(
                    "expected closing '}}' in format spec at offset {}; found '{}'",
                    location, n as char
                )
            } else {
                format!("missing closing '}}' in format spec at offset {}", location)
            };
            Err(Error::new(ErrorKind::InvalidOperation, err))
        }
    }

    fn parse_field_name<'s>(cursor: &mut Cursor<'s>) -> Result<Option<FieldName<'s>>, Error> {
        if let Some(num) = parse_number(cursor)? {
            Ok(Some(FieldName::Positional(num, parse_path(cursor)?)))
        } else if let Some(ident) = parse_identifier(cursor) {
            Ok(Some(FieldName::Kwarg(ident, parse_path(cursor)?)))
        } else {
            Ok(None)
        }
    }

    fn parse_path<'s>(cursor: &mut Cursor<'s>) -> Result<Vec<PathElem<'s>>, Error> {
        let mut elems = Vec::new();
        loop {
            if cursor.advance_if(b'.') {
                if let Some(attr) = parse_identifier(cursor) {
                    elems.push(PathElem::Attr(attr));
                } else {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        format!(
                            "missing attribute name after '.' in format spec at offset {}",
                            cursor.position()
                        ),
                    ));
                }
            } else if cursor.advance_if(b'[') {
                let key = parse_till(cursor, b']')?;
                elems.push(PathElem::Key(key))
            } else {
                break;
            }
        }
        Ok(elems)
    }

    fn parse_identifier<'s>(cursor: &mut Cursor<'s>) -> Option<&'s str> {
        let ident_chars = cursor
            .rest_bytes()
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

        if ident_chars > 0 {
            Some(cursor.advance(ident_chars))
        } else {
            None
        }
    }

    fn parse_format_spec(cursor: &mut Cursor) -> Result<FormatSpec, Error> {
        let location = cursor.position();
        let mut print_sign = false;
        let mut space_before_positive_num = false;
        let fill_align = parse_fill_align(cursor);

        if cursor.advance_if(b'+') {
            print_sign = true;
        } else if cursor.advance_if(b' ') {
            space_before_positive_num = true;
        } else {
            cursor.advance_if(b'-');
        }

        let alternate_form = cursor.advance_if(b'#');
        let mut zero_padded = cursor.advance_if(b'0');

        let mut width = parse_number(cursor)?;
        if zero_padded && width.is_none() {
            // if '0' is not followed by width (i.e. digit+), then it should be parsed as
            // a width, not as zero-padding.
            zero_padded = false;
            width = Some(0);
        }

        let precision = cursor
            .advance_if(b'.')
            .then(|| parse_number(cursor))
            .transpose()?
            .flatten();

        let ty = parse_type(cursor, FormatStyle::StrFormat)?;
        Ok(FormatSpec {
            fill_align,
            print_sign,
            space_before_positive_num,
            alternate_form,
            zero_padded,
            width,
            precision,
            ty,
            format_style: FormatStyle::StrFormat,
            location,
        })
    }

    fn parse_fill_align(cursor: &mut Cursor) -> Option<FillAlign> {
        let maybe_fill = cursor.rest().chars().next();
        let maybe_align = cursor.rest().chars().nth(1);

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
            (_, _) => return None,
        };

        cursor.advance(consumed);
        Some(fa)
    }

    fn get_nested_val(root: &Value, path: &[PathElem]) -> Result<Value, Error> {
        let mut curr = root.clone();
        for elem in path {
            curr = match elem {
                PathElem::Attr(attr) => curr.get_attr(attr)?,
                PathElem::Key(index) => {
                    if let Ok(num) = index.parse::<usize>() {
                        curr.get_item_by_index(num)?
                    } else {
                        curr.get_attr(index)?
                    }
                }
            };
        }
        if curr.is_undefined() {
            Err(Error::from(ErrorKind::UndefinedError))
        } else {
            Ok(curr)
        }
    }

    // Do str.format style formatting. Parse the format string and apply values from
    // args to the fields found in the string, by formatting the value according to
    // the spec found in the field.
    pub fn format(format_str: &str, args: &[Value]) -> Result<String, Error> {
        let mut input = Tokenizer::new(format_str, FormatStyle::StrFormat);
        let mut result = String::new();

        fn missing_arg_err(location: usize, source: Option<Error>) -> Error {
            let err = Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "argument not found for format field at offset {}",
                    location
                ),
            );

            if let Some(cause) = source {
                err.with_source(cause)
            } else {
                err
            }
        }

        fn switch_err(location: usize, from: &str, to: &str) -> Error {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("cannot switch from {from} to {to} in field at offset {location}"),
            )
        }

        let (args, kwargs): (&[Value], Kwargs) = from_args(args)?;
        let mut arg_index = 0;
        let mut auto_numbering = false;
        let mut manual_numbering = false;

        while let Some(token) = input.next_token()? {
            match token {
                Token::Literal(lit) => result.push_str(lit),
                Token::Replace(ReplacementField {
                    field_name,
                    format_spec,
                    location,
                }) => {
                    // find the right argument to replace the field with
                    let arg = match field_name {
                        Some(FieldName::Kwarg(key, path)) => {
                            let val = kwargs
                                .peek::<Value>(key)
                                .map_err(|e| missing_arg_err(location, Some(e)))?;
                            get_nested_val(&val, &path)
                                .map_err(|e| missing_arg_err(location, Some(e)))?
                        }
                        Some(FieldName::Positional(index, path)) => {
                            manual_numbering = true;
                            if auto_numbering {
                                return Err(switch_err(
                                    location,
                                    "automatic numbering",
                                    "manual field specification",
                                ));
                            }
                            let val = args
                                .get(index)
                                .ok_or_else(|| missing_arg_err(location, None))?;
                            get_nested_val(val, &path)
                                .map_err(|e| missing_arg_err(location, Some(e)))?
                        }
                        None => {
                            auto_numbering = true;
                            if manual_numbering {
                                return Err(switch_err(
                                    location,
                                    "manual field specification",
                                    "automatic numbering",
                                ));
                            }
                            let val = args
                                .get(arg_index)
                                .ok_or_else(|| missing_arg_err(location, None))?;
                            arg_index += 1;
                            val.clone()
                        }
                        Some(FieldName::MappingKey(_)) => unreachable!(),
                    };

                    // apply the spec to the replacement value, and insert the
                    // formatted result into final string
                    if let Some(spec) = format_spec {
                        result.push_str(&spec.format(&arg)?);
                    } else {
                        result.push_str(&format!("{arg}"))
                    }
                }
            }
        }
        Ok(result)
    }
}
