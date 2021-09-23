use std::borrow::Cow;
use std::fmt;

/// Represents a token in the stream.
pub enum Token<'a> {
    /// Raw template data.
    TemplateData(&'a str),
    /// Variable block start (with or without whitespace removal).
    VariableStart(bool),
    /// Variable block start (with or without whitespace removal).
    VariableEnd(bool),
    /// Statement block start (with or without whitespace removal).
    BlockStart(bool),
    /// Statement block start (with or without whitespace removal).
    BlockEnd(bool),
    /// An identifier.
    Ident(&'a str),
    /// A string.
    Str(Cow<'a, str>),
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

impl<'a> fmt::Debug for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::TemplateData(s) => write!(f, "TEMPLATE_DATA({:?})", s),
            Token::VariableStart(ws) => write!(f, "VARIABLE_START({:?})", ws),
            Token::VariableEnd(ws) => write!(f, "VARIABLE_END({:?})", ws),
            Token::BlockStart(ws) => write!(f, "BLOCK_END({:?})", ws),
            Token::BlockEnd(ws) => write!(f, "BLOCK_END({:?})", ws),
            Token::Ident(i) => write!(f, "IDENT({})", i),
            Token::Str(s) => write!(f, "STR({:?})", s),
            Token::Int(i) => write!(f, "INT({:?})", i),
            Token::Float(v) => write!(f, "FLOAT({:?})", v),
            Token::Plus => write!(f, "PLUS"),
            Token::Minus => write!(f, "MINUS"),
            Token::Mul => write!(f, "MUL"),
            Token::Div => write!(f, "DIV"),
            Token::FloorDiv => write!(f, "FLOORDIV"),
            Token::Pow => write!(f, "POW"),
            Token::Mod => write!(f, "MOD"),
            Token::Bang => write!(f, "BANG"),
            Token::Dot => write!(f, "DOT"),
            Token::Comma => write!(f, "COMMA"),
            Token::Colon => write!(f, "COLON"),
            Token::Tilde => write!(f, "TILDE"),
            Token::Assign => write!(f, "ASSIGN"),
            Token::Pipe => write!(f, "PIPE"),
            Token::Eq => write!(f, "EQ"),
            Token::Ne => write!(f, "NE"),
            Token::Gt => write!(f, "GT"),
            Token::Gte => write!(f, "GTE"),
            Token::Lt => write!(f, "LT"),
            Token::Lte => write!(f, "LTE"),
            Token::BracketOpen => write!(f, "BRACKET_OPEN"),
            Token::BracketClose => write!(f, "BRACKET_CLOSE"),
            Token::ParenOpen => write!(f, "PAREN_OPEN"),
            Token::ParenClose => write!(f, "PAREN_CLOSE"),
            Token::BraceOpen => write!(f, "BRACE_OPEN"),
            Token::BraceClose => write!(f, "BRACE_CLOSE"),
        }
    }
}

impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::TemplateData(_) => write!(f, "template-data"),
            Token::VariableStart(_) => write!(f, "start of variable block"),
            Token::VariableEnd(_) => write!(f, "end of variable block"),
            Token::BlockStart(_) => write!(f, "start of block"),
            Token::BlockEnd(_) => write!(f, "end of block"),
            Token::Ident(_) => write!(f, "identifier"),
            Token::Str(_) => write!(f, "string"),
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
#[derive(Clone, Copy, Default)]
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
