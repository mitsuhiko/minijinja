use std::fmt;

/// Represents a token in the stream.
#[derive(Debug)]
#[cfg_attr(
    feature = "unstable_machinery_serde",
    derive(serde::Serialize),
    serde(tag = "name", content = "payload")
)]
pub enum Token<'a> {
    /// Raw template data.
    TemplateData(&'a str),
    /// Variable block start.
    VariableStart,
    /// Variable block end
    VariableEnd,
    /// Statement block start
    BlockStart,
    /// Statement block end
    BlockEnd,
    /// An identifier.
    Ident(&'a str),
    /// A borrowed string.
    Str(&'a str),
    /// An allocated string.
    String(String),
    /// An integer (limited to i64)
    Int(i64),
    /// A float
    Float(f64),
    /// A plus (`+`) operator.
    Plus,
    /// A plus (`-`) operator.
    Minus,
    /// A mul (`*`) operator.
    Mul,
    /// A div (`/`) operator.
    Div,
    /// A floor division (`//`) operator.
    FloorDiv,
    /// Power operator (`**`).
    Pow,
    /// A mod (`%`) operator.
    Mod,
    /// The bang (`!`) operator.
    Bang,
    /// A dot operator (`.`)
    Dot,
    /// The comma operator (`,`)
    Comma,
    /// The colon operator (`:`)
    Colon,
    /// The tilde operator (`~`)
    Tilde,
    /// The assignment operator (`=`)
    Assign,
    /// The pipe symbol.
    Pipe,
    /// `==` operator
    Eq,
    /// `!=` operator
    Ne,
    /// `>` operator
    Gt,
    /// `>=` operator
    Gte,
    /// `<` operator
    Lt,
    /// `<=` operator
    Lte,
    /// Open Bracket
    BracketOpen,
    /// Close Bracket
    BracketClose,
    /// Open Parenthesis
    ParenOpen,
    /// Close Parenthesis
    ParenClose,
    /// Open Brace
    BraceOpen,
    /// Close Brace
    BraceClose,
}

impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::TemplateData(_) => write!(f, "template-data"),
            Token::VariableStart => write!(f, "start of variable block"),
            Token::VariableEnd => write!(f, "end of variable block"),
            Token::BlockStart => write!(f, "start of block"),
            Token::BlockEnd => write!(f, "end of block"),
            Token::Ident(_) => write!(f, "identifier"),
            Token::Str(_) | Token::String(_) => write!(f, "string"),
            Token::Int(_) => write!(f, "integer"),
            Token::Float(_) => write!(f, "float"),
            Token::Plus => write!(f, "`+`"),
            Token::Minus => write!(f, "`-`"),
            Token::Mul => write!(f, "`*`"),
            Token::Div => write!(f, "`/`"),
            Token::FloorDiv => write!(f, "`//`"),
            Token::Pow => write!(f, "`**`"),
            Token::Mod => write!(f, "`%`"),
            Token::Bang => write!(f, "`!`"),
            Token::Dot => write!(f, "`.`"),
            Token::Comma => write!(f, "`,`"),
            Token::Colon => write!(f, "`:`"),
            Token::Tilde => write!(f, "`~`"),
            Token::Assign => write!(f, "`=`"),
            Token::Pipe => write!(f, "`|`"),
            Token::Eq => write!(f, "`==`"),
            Token::Ne => write!(f, "`!=`"),
            Token::Gt => write!(f, "`>`"),
            Token::Gte => write!(f, "`>=`"),
            Token::Lt => write!(f, "`<`"),
            Token::Lte => write!(f, "`<=`"),
            Token::BracketOpen => write!(f, "`[`"),
            Token::BracketClose => write!(f, "`]`"),
            Token::ParenOpen => write!(f, "`(`"),
            Token::ParenClose => write!(f, "`)`"),
            Token::BraceOpen => write!(f, "`{{`"),
            Token::BraceClose => write!(f, "`}}`"),
        }
    }
}

/// Token span information
#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "unstable_machinery_serde", derive(serde::Serialize))]
pub struct Span {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            " @ {}:{}-{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}
