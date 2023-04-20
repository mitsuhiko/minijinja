use std::borrow::Cow;
use std::fmt;

use crate::compiler::tokens::Span;

/// Represents template errors.
///
/// If debug mode is enabled a template error contains additional debug
/// information that can be displayed by formatting an error with the
/// alternative formatting (``format!("{:#}", err)``).  That information
/// is also shown for the [`Debug`] display where the extended information
/// is hidden when the alternative formatting is used.
///
/// Since MiniJinja takes advantage of chained errors it's recommended
/// to render the entire chain to better understand the causes.
///
/// # Example
///
/// Here is an example of you might want to render errors:
///
/// ```rust
/// # let mut env = minijinja::Environment::new();
/// # env.add_template("", "");
/// # let template = env.get_template("").unwrap(); let ctx = ();
/// match template.render(ctx) {
///     Ok(result) => println!("{}", result),
///     Err(err) => {
///         eprintln!("Could not render template: {:#}", err);
///         // render causes as well
///         let mut err = &err as &dyn std::error::Error;
///         while let Some(next_err) = err.source() {
///             eprintln!();
///             eprintln!("caused by: {:#}", next_err);
///             err = next_err;
///         }
///     }
/// }
/// ```
pub struct Error {
    repr: Box<ErrorRepr>,
}

/// The internal error data
struct ErrorRepr {
    kind: ErrorKind,
    detail: Option<Cow<'static, str>>,
    name: Option<String>,
    lineno: usize,
    span: Option<Span>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    #[cfg(feature = "debug")]
    debug_info: Option<crate::debug::DebugInfo>,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut err = f.debug_struct("Error");
        err.field("kind", &self.kind());
        if let Some(ref detail) = self.repr.detail {
            err.field("detail", detail);
        }
        if let Some(ref name) = self.name() {
            err.field("name", name);
        }
        if let Some(line) = self.line() {
            err.field("line", &line);
        }
        if let Some(ref source) = std::error::Error::source(self) {
            err.field("source", source);
        }
        ok!(err.finish());

        // so this is a bit questionablem, but because of how commonly errors are just
        // unwrapped i think it's sensible to spit out the debug info following the
        // error struct dump.
        #[cfg(feature = "debug")]
        {
            if !f.alternate() {
                if let Some(info) = self.debug_info() {
                    ok!(writeln!(f));
                    ok!(crate::debug::render_debug_info(
                        f,
                        self.name(),
                        self.kind(),
                        self.line(),
                        self.span(),
                        info,
                    ));
                    ok!(writeln!(f));
                }
            }
        }

        Ok(())
    }
}

/// An enum describing the error kind.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// A non primitive value was encountered where one was expected.
    NonPrimitive,
    /// A value is not valid for a key in a map.
    NonKey,
    /// An invalid operation was attempted.
    InvalidOperation,
    /// The template has a syntax error
    SyntaxError,
    /// A template was not found.
    TemplateNotFound,
    /// Too many arguments were passed to a function.
    TooManyArguments,
    /// A expected argument was missing
    MissingArgument,
    /// A filter is unknown
    UnknownFilter,
    /// A test is unknown
    UnknownTest,
    /// A function is unknown
    UnknownFunction,
    /// Un unknown method was called
    UnknownMethod,
    /// A bad escape sequence in a string was encountered.
    BadEscape,
    /// An operation on an undefined value was attempted.
    UndefinedError,
    /// Not able to serialize this value.
    BadSerialization,
    /// An error happened in an include.
    BadInclude,
    /// An error happened in a super block.
    EvalBlock,
    /// Unable to unpack a value.
    CannotUnpack,
    /// Failed writing output.
    WriteFailure,
    /// Engine ran out of fuel
    #[cfg(feature = "fuel")]
    OutOfFuel,
    #[cfg(feature = "custom_syntax")]
    /// Error creating aho-corasick delimiters
    InvalidDelimiter,
}

impl ErrorKind {
    fn description(self) -> &'static str {
        match self {
            ErrorKind::NonPrimitive => "not a primitive",
            ErrorKind::NonKey => "not a key type",
            ErrorKind::InvalidOperation => "invalid operation",
            ErrorKind::SyntaxError => "syntax error",
            ErrorKind::TemplateNotFound => "template not found",
            ErrorKind::TooManyArguments => "too many arguments",
            ErrorKind::MissingArgument => "missing argument",
            ErrorKind::UnknownFilter => "unknown filter",
            ErrorKind::UnknownFunction => "unknown function",
            ErrorKind::UnknownTest => "unknown test",
            ErrorKind::UnknownMethod => "unknown method",
            ErrorKind::BadEscape => "bad string escape",
            ErrorKind::UndefinedError => "undefined value",
            ErrorKind::BadSerialization => "could not serialize to value",
            ErrorKind::BadInclude => "could not render include",
            ErrorKind::EvalBlock => "could not render block",
            ErrorKind::CannotUnpack => "cannot unpack",
            ErrorKind::WriteFailure => "failed to write output",
            #[cfg(feature = "fuel")]
            ErrorKind::OutOfFuel => "engine ran out of fuel",
            #[cfg(feature = "custom_syntax")]
            ErrorKind::InvalidDelimiter => "invalid custom delimiters",
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
        if let Some(ref detail) = self.repr.detail {
            ok!(write!(f, "{}: {}", self.kind(), detail));
        } else {
            ok!(write!(f, "{}", self.kind()));
        }
        if let Some(ref filename) = self.name() {
            ok!(write!(f, " (in {}:{})", filename, self.line().unwrap_or(0)))
        }
        #[cfg(feature = "debug")]
        {
            if f.alternate() {
                if let Some(info) = self.debug_info() {
                    ok!(crate::debug::render_debug_info(
                        f,
                        self.name(),
                        self.kind(),
                        self.line(),
                        self.span(),
                        info,
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Error {
    /// Creates a new error with kind and detail.
    pub fn new<D: Into<Cow<'static, str>>>(kind: ErrorKind, detail: D) -> Error {
        Error {
            repr: Box::new(ErrorRepr {
                kind,
                detail: Some(detail.into()),
                name: None,
                lineno: 0,
                span: None,
                source: None,
                #[cfg(feature = "debug")]
                debug_info: None,
            }),
        }
    }

    pub(crate) fn set_filename_and_line(&mut self, filename: &str, lineno: usize) {
        self.repr.name = Some(filename.into());
        self.repr.lineno = lineno;
    }

    pub(crate) fn set_filename_and_span(&mut self, filename: &str, span: Span) {
        self.repr.name = Some(filename.into());
        self.repr.span = Some(span);
        self.repr.lineno = span.start_line as usize;
    }

    pub(crate) fn new_not_found(name: &str) -> Error {
        Error::new(
            ErrorKind::TemplateNotFound,
            format!("template {name:?} does not exist"),
        )
    }

    /// Attaches another error as source to this error.
    #[allow(unused)]
    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, source: E) -> Self {
        self.repr.source = Some(Box::new(source));
        self
    }

    /// Returns the error kind
    pub fn kind(&self) -> ErrorKind {
        self.repr.kind
    }

    /// Returns the filename of the template that caused the error.
    pub fn name(&self) -> Option<&str> {
        self.repr.name.as_deref()
    }

    /// Returns the line number where the error occurred.
    pub fn line(&self) -> Option<usize> {
        if self.repr.lineno > 0 {
            Some(self.repr.lineno)
        } else {
            None
        }
    }

    /// Returns the line number where the error occurred.
    #[cfg(feature = "debug")]
    pub(crate) fn span(&self) -> Option<Span> {
        self.repr.span
    }

    /// Returns the template debug information is available.
    ///
    /// The debug info snapshot is only embedded into the error if the debug
    /// mode is enabled on the environment
    /// ([`Environment::set_debug`](crate::Environment::set_debug)).
    #[cfg(feature = "debug")]
    #[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
    pub(crate) fn debug_info(&self) -> Option<&crate::debug::DebugInfo> {
        self.repr.debug_info.as_ref()
    }

    #[cfg(feature = "debug")]
    #[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
    pub(crate) fn attach_debug_info(&mut self, value: crate::debug::DebugInfo) {
        self.repr.debug_info = Some(value);
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.repr.source.as_ref().map(|err| err.as_ref() as _)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error {
            repr: Box::new(ErrorRepr {
                kind,
                detail: None,
                name: None,
                lineno: 0,
                span: None,
                source: None,
                #[cfg(feature = "debug")]
                debug_info: None,
            }),
        }
    }
}

impl From<fmt::Error> for Error {
    fn from(_: fmt::Error) -> Self {
        Error::new(ErrorKind::WriteFailure, "formatting failed")
    }
}

pub fn attach_basic_debug_info<T>(rv: Result<T, Error>, source: &str) -> Result<T, Error> {
    #[cfg(feature = "debug")]
    {
        match rv {
            Ok(rv) => Ok(rv),
            Err(mut err) => {
                err.repr.debug_info = Some(crate::debug::DebugInfo {
                    template_source: Some(source.to_string()),
                    ..Default::default()
                });
                Err(err)
            }
        }
    }
    #[cfg(not(feature = "debug"))]
    {
        let _source = source;
        rv
    }
}
