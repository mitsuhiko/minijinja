use std::borrow::Cow;
use std::fmt;

/// Represents template errors.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    detail: Option<Cow<'static, str>>,
    name: Option<String>,
    lineno: usize,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind()
    }
}

impl Eq for Error {}

/// An enum describing the error kind.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidSyntax,
    NonPrimitive,
    NonKey,
    ImpossibleOperation,
    InvalidOperation,
    SyntaxError,
    TemplateNotFound,
    InvalidArguments,
    UnknownFilter,
    UnknownTest,
    BadEscape,
    UndefinedError,
    BadSerialization,
}

impl ErrorKind {
    fn description(self) -> &'static str {
        match self {
            ErrorKind::InvalidSyntax => "invalid syntax",
            ErrorKind::NonPrimitive => "not a primitive",
            ErrorKind::NonKey => "not a key type",
            ErrorKind::ImpossibleOperation => "impossible operation",
            ErrorKind::InvalidOperation => "invalid operation",
            ErrorKind::SyntaxError => "syntax error",
            ErrorKind::TemplateNotFound => "template not found",
            ErrorKind::InvalidArguments => "invalid arguments",
            ErrorKind::UnknownFilter => "unknown filter",
            ErrorKind::UnknownTest => "unknown test",
            ErrorKind::BadEscape => "bad string escape",
            ErrorKind::UndefinedError => "variable or attribute undefined",
            ErrorKind::BadSerialization => "could not serialize to internal format",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref detail) = self.detail {
            write!(f, "{}: {}", self.kind, detail)?;
        } else {
            write!(f, "{}", self.kind)?;
        }
        if let Some(ref filename) = self.name {
            write!(f, " (in {}:{})", filename, self.lineno)?
        }
        Ok(())
    }
}

impl Error {
    /// Creates a new error with kind and detail.
    pub fn new<D: Into<Cow<'static, str>>>(kind: ErrorKind, detail: D) -> Error {
        Error {
            kind,
            detail: Some(detail.into()),
            name: None,
            lineno: 0,
            source: None,
        }
    }

    pub(crate) fn set_location(&mut self, filename: &str, lineno: usize) {
        self.name = Some(filename.into());
        self.lineno = lineno;
    }

    /// Attaches another error as source to this error.
    #[allow(unused)]
    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, source: E) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Returns the error kind
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the filename.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the line.
    pub fn line(&self) -> Option<usize> {
        self.name.as_ref().map(|_| self.lineno)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|err| err.as_ref() as _)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error {
            kind,
            detail: None,
            name: None,
            lineno: 0,
            source: None,
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::new(ErrorKind::BadSerialization, msg.to_string())
    }
}
