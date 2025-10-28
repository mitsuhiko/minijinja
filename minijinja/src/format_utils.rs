use crate::value::{Rest, ValueKind};
use crate::{Error, ErrorKind, Value};

// Token produced by the format string parser
#[derive(Debug)]
enum Token<'src> {
    // Text that must be copied verbatim
    Literal(&'src str),
    // Field that must be replaced with formatted text
    Replace(ReplacementField<'src>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FieldName<'src> {
    Ident(&'src str),
    Digit(usize),
    MappingKey(&'src str),
}

#[derive(Debug, PartialEq, Eq)]
struct ReplacementField<'src> {
    field_name: Option<FieldName<'src>>,
    format_spec: Option<FormatSpec>,
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
    printf_style: bool,
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
            Type::String => {
                if self.printf_style {
                    // Format "true" or "false" as a regular string, ignoring the
                    // precision (i.e. without truncating)
                    Ok(self.apply_padding(format!("{val}"), Align::Right))
                } else {
                    Err(self.type_conversion_err("bool", Type::String))
                }
            }
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
                let default_align = if self.printf_style {
                    Align::Right
                } else {
                    Align::Left
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
                if self.printf_style {
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
            Type::String if !self.printf_style => {
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

// Type representing a printf-style format string input. It provides token interface
// over the raw string through its `next_token` method.
struct PrintfStyleInput<'s> {
    source: &'s str,
    current_offset: usize,
}

impl<'s> PrintfStyleInput<'s> {
    fn new(source: &'s str) -> Self {
        Self {
            source,
            current_offset: 0,
        }
    }

    fn next_token(&mut self) -> Result<Option<Token<'s>>, Error> {
        let mut offset = 0;
        let mut found_spec = false;
        let bytes = self.rest_bytes();
        loop {
            match bytes.get(offset) {
                Some(b'%') => {
                    // check for escape sequence
                    if let Some(b'%') = bytes.get(offset + 1) {
                        // jump over %%
                        offset += 2;
                    } else {
                        // start of format spec, break without jumping the %
                        found_spec = true;
                        break;
                    }
                }
                Some(_) => {
                    offset += 1;
                }
                None => break,
            }
        }
        if offset > 0 {
            let tok = Token::Literal(self.advance(offset));
            Ok(Some(tok))
        } else if found_spec {
            let tok = Token::Replace(self.replacement_field()?);
            Ok(Some(tok))
        } else {
            Ok(None)
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

    fn is_end(&self) -> bool {
        self.source.len() == self.current_offset
    }

    // Top-level parser using the following grammar:
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
    fn replacement_field(&mut self) -> Result<ReplacementField<'s>, Error> {
        // consume '%'
        self.advance(1);

        let field_name = self.key()?.map(FieldName::MappingKey);
        let spec = self.format_spec()?;
        Ok(ReplacementField {
            field_name,
            format_spec: Some(spec),
        })
    }

    fn key(&mut self) -> Result<Option<&'s str>, Error> {
        if self.advance_if(b'(') {
            let start = self.current_offset;
            loop {
                if self.advance_if(b')') {
                    break;
                } else if self.is_end() {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        format!(
                            "incomplete format key at offset {}; missing enclosing ')'",
                            start
                        ),
                    ));
                } else {
                    self.advance(1);
                }
            }
            // don't include the closing ')' in the key
            let end = self.current_offset - 1;
            Ok(Some(&self.source[start..end]))
        } else {
            Ok(None)
        }
    }

    fn format_spec(&mut self) -> Result<FormatSpec, Error> {
        let location = self.current_offset;
        let printf_style = true;
        let mut fill_align = None;
        let mut print_sign = false;
        let mut space_before_positive_num = false;
        let mut alternate_form = false;
        let mut zero_padded = false;

        loop {
            match self.rest_bytes().get(0) {
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
            self.advance(1);
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

        let mut width = self.number()?;
        if zero_padded && width.is_none() {
            // if '0' is not followed by width (i.e. digit+), then it should be parsed as
            // a width, not as zero-padding.
            zero_padded = false;
            width = Some(0);
        }

        let precision = self
            .advance_if(b'.')
            .then(|| self.number())
            .transpose()?
            .flatten();

        // length modifier is ignored in Python
        self.len_modifier();
        let ty = self.typ()?;
        Ok(FormatSpec {
            fill_align,
            print_sign,
            space_before_positive_num,
            alternate_form,
            zero_padded,
            width,
            precision,
            ty,
            printf_style,
            location,
        })
    }

    fn len_modifier(&mut self) {
        match self.rest_bytes().get(0) {
            Some(b'h') | Some(b'l') | Some(b'L') => {
                self.advance(1);
            }
            _ => (),
        }
    }

    fn number(&mut self) -> Result<Option<usize>, Error> {
        let digit_count = self
            .rest_bytes()
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digit_count == 0 {
            Ok(None)
        } else {
            let num_str = self.advance(digit_count);
            let num = num_str.parse::<usize>().map_err(|e| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!(
                        "invalid integer in the format string at offset {}",
                        self.current_offset
                    ),
                )
                .with_source(e)
            })?;
            Ok(Some(num))
        }
    }

    fn typ(&mut self) -> Result<Type, Error> {
        let t = match self.rest_bytes().get(0) {
            Some(b'd') => Type::Decimal,
            Some(b'i') => Type::Decimal,
            Some(b'e') => Type::LowerE,
            Some(b'E') => Type::UpperE,
            Some(b'f') => Type::LowerF,
            Some(b'F') => Type::UpperF,
            Some(b'o') => Type::Octal,
            Some(b'x') => Type::LowerHex,
            Some(b'X') => Type::UpperHex,
            Some(b's') => Type::String,
            Some(c) => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!(
                        "invalid conversion type '{}' in format spec at offset {}",
                        *c as char, self.current_offset
                    ),
                ))
            }
            None => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!(
                        "incomplete format spec at offset {}; missing conversion type",
                        self.current_offset
                    ),
                ))
            }
        };
        self.advance(1);
        Ok(t)
    }
}

pub fn printf_style_format(format_str: Value, format_args: Rest<Value>) -> Result<String, Error> {
    let format_str = format_str.as_str().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "format filter argument must be a string, found {}",
                format_str.kind()
            ),
        )
    })?;

    let mut input = PrintfStyleInput::new(format_str);
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
            }) => {
                let spec = format_spec.expect("printf-style format must specify a spec");
                let arg = {
                    if let Some(FieldName::MappingKey(key)) = field_name {
                        // only a mapping as an argument is expected, and the key must be
                        // read from the provided mapping.
                        if let Some(arg) = format_args.0.get(0) {
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
                    } else if let Some(arg) = format_args.0.get(arg_index) {
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
