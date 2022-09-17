use std::borrow::Cow;
use std::fmt;

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
    kind: ErrorKind,
    detail: Option<Cow<'static, str>>,
    name: Option<String>,
    lineno: usize,
    span: Option<Span>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    #[cfg(feature = "debug")]
    pub(crate) debug_info: Option<DebugInfo>,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut err = f.debug_struct("Error");
        err.field("kind", &self.kind);
        if let Some(ref detail) = self.detail {
            err.field("detail", detail);
        }
        if let Some(ref name) = self.name {
            err.field("name", name);
        }
        if self.lineno > 0 {
            err.field("line", &self.lineno);
        }
        if let Some(ref source) = self.source {
            err.field("source", source);
        }
        err.finish()?;

        // so this is a bit questionablem, but because of how commonly errors are just
        // unwrapped i think it's sensible to spit out the debug info following the
        // error struct dump.
        #[cfg(feature = "debug")]
        {
            if !f.alternate() {
                if let Some(info) = self.debug_info() {
                    writeln!(f)?;
                    render_debug_info(f, self.kind, self.line(), self.span, info)?;
                    writeln!(f)?;
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
    /// An impossible operation was attempted.
    ImpossibleOperation,
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
    /// A bad escape sequence in a string was encountered.
    BadEscape,
    /// An operation on an undefined value was attempted.
    UndefinedError,
    /// Impossible to serialize this value.
    BadSerialization,
    /// An error happened in an include.
    BadInclude,
    /// An error happened in a super block.
    EvalBlock,
    /// Unable to unpack a value.
    CannotUnpack,
    /// Failed writing output.
    WriteFailure,
}

impl ErrorKind {
    fn description(self) -> &'static str {
        match self {
            ErrorKind::NonPrimitive => "not a primitive",
            ErrorKind::NonKey => "not a key type",
            ErrorKind::ImpossibleOperation => "impossible operation",
            ErrorKind::SyntaxError => "syntax error",
            ErrorKind::TemplateNotFound => "template not found",
            ErrorKind::TooManyArguments => "too many arguments",
            ErrorKind::MissingArgument => "missing argument",
            ErrorKind::UnknownFilter => "unknown filter",
            ErrorKind::UnknownFunction => "unknown function",
            ErrorKind::UnknownTest => "unknown test",
            ErrorKind::BadEscape => "bad string escape",
            ErrorKind::UndefinedError => "undefined value",
            ErrorKind::BadSerialization => "could not serialize to internal format",
            ErrorKind::BadInclude => "could not render an included template",
            ErrorKind::EvalBlock => "could not render block",
            ErrorKind::CannotUnpack => "cannot unpack",
            ErrorKind::WriteFailure => "failed to write output",
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
        #[cfg(feature = "debug")]
        {
            if f.alternate() {
                if let Some(info) = self.debug_info() {
                    render_debug_info(f, self.kind, self.line(), self.span, info)?;
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
            kind,
            detail: Some(detail.into()),
            name: None,
            lineno: 0,
            span: None,
            source: None,
            #[cfg(feature = "debug")]
            debug_info: None,
        }
    }

    pub(crate) fn set_filename_and_line(&mut self, filename: &str, lineno: usize) {
        self.name = Some(filename.into());
        self.lineno = lineno;
    }

    pub(crate) fn set_filename_and_span(&mut self, filename: &str, span: Span) {
        self.name = Some(filename.into());
        self.span = Some(span);
        self.lineno = span.start_line;
    }

    pub(crate) fn new_not_found(name: &str) -> Error {
        Error::new(
            ErrorKind::TemplateNotFound,
            format!("template {:?} does not exist", name),
        )
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

    /// Returns the template debug information is available.
    ///
    /// The debug info snapshot is only embedded into the error if the debug
    /// mode is enabled on the environment
    /// ([`Environment::set_debug`](crate::Environment::set_debug)).
    #[cfg(feature = "debug")]
    #[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
    pub(crate) fn debug_info(&self) -> Option<&DebugInfo> {
        self.debug_info.as_ref()
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
            span: None,
            source: None,
            #[cfg(feature = "debug")]
            debug_info: None,
        }
    }
}

impl From<fmt::Error> for Error {
    fn from(_: fmt::Error) -> Self {
        Error::new(ErrorKind::WriteFailure, "formatting failed")
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

#[cfg(feature = "debug")]
mod debug_info {
    use super::*;
    use crate::value::Value;

    /// This is a snapshot of the debug information.
    #[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
    #[derive(Default)]
    pub(crate) struct DebugInfo {
        pub(crate) template_source: Option<String>,
        pub(crate) context: Option<Value>,
        pub(crate) referenced_names: Option<Vec<String>>,
    }

    struct VarPrinter<'x>(Value, &'x [String]);

    impl<'x> fmt::Debug for VarPrinter<'x> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut m = f.debug_struct("Referenced variables:");
            for var in self.1 {
                match self.0.get_attr(var) {
                    Ok(val) => m.field(var, &val),
                    Err(_) => m.field(var, &Value::UNDEFINED),
                };
            }
            m.finish()
        }
    }

    impl DebugInfo {
        /// If available this contains a reference to the source string.
        pub fn source(&self) -> Option<&str> {
            self.template_source.as_deref()
        }

        /// Provides access to a snapshot of the context.
        ///
        /// The context is created at the time the error was created if that error
        /// happened during template rendering.
        pub fn context(&self) -> Option<Value> {
            self.context.clone()
        }

        /// Returns a narrowed down set of referenced names from the context
        /// where the error happened.
        ///
        /// This function is currently internal and only used for the default
        /// error printing.  This could be exposed but it's a highly specific
        /// API.
        pub(crate) fn referenced_names(&self) -> Option<&[String]> {
            self.referenced_names.as_deref()
        }
    }

    pub(super) fn render_debug_info(
        f: &mut fmt::Formatter,
        kind: ErrorKind,
        line: Option<usize>,
        span: Option<Span>,
        info: &DebugInfo,
    ) -> fmt::Result {
        if let Some(source) = info.source() {
            writeln!(f)?;
            writeln!(f, "{:-^1$}", " Template Source ", 74).unwrap();
            let lines: Vec<_> = source.lines().enumerate().collect();
            let idx = line.unwrap_or(1).saturating_sub(1);
            let skip = idx.saturating_sub(3);
            let pre = lines.iter().skip(skip).take(3.min(idx)).collect::<Vec<_>>();
            let post = lines.iter().skip(idx + 1).take(3).collect::<Vec<_>>();
            for (idx, line) in pre {
                writeln!(f, "{:>4} | {}", idx + 1, line).unwrap();
            }

            writeln!(f, "{:>4} > {}", idx + 1, lines[idx].1).unwrap();
            if let Some(span) = span {
                if span.start_line == span.end_line {
                    writeln!(
                        f,
                        "     i {}{} {}",
                        " ".repeat(span.start_col),
                        "^".repeat(span.end_col - span.start_col),
                        kind,
                    )?;
                }
            }

            for (idx, line) in post {
                writeln!(f, "{:>4} | {}", idx + 1, line).unwrap();
            }
            write!(f, "{:-^1$}", "", 74).unwrap();
        }
        if let Some(ctx) = info.context() {
            if let Some(vars) = info.referenced_names() {
                writeln!(f)?;
                writeln!(f, "{:#?}", VarPrinter(ctx, vars))?;
            }
            write!(f, "{:-^1$}", "", 74).unwrap();
        }
        Ok(())
    }
}

pub fn attach_basic_debug_info<T>(rv: Result<T, Error>, source: &str) -> Result<T, Error> {
    #[cfg(feature = "debug")]
    {
        match rv {
            Ok(rv) => Ok(rv),
            Err(mut err) => {
                err.debug_info = Some(crate::error::DebugInfo {
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

use crate::compiler::tokens::Span;

#[cfg(feature = "debug")]
pub(crate) use self::debug_info::*;
