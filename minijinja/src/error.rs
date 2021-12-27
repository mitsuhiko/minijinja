use std::borrow::Cow;
use std::fmt;

/// Represents template errors.
///
/// If debug mode is enabled a template error contains additional debug
/// information that can be displayed by formatting an error with the
/// alternative formatting (``format!("{:#}", err)``).
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
///         eprintln!("Could not render template:");
///         eprintln!("  {:#}", err);
///     }
/// }
/// ```
pub struct Error {
    kind: ErrorKind,
    detail: Option<Cow<'static, str>>,
    name: Option<String>,
    lineno: usize,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    #[cfg(feature = "debug")]
    pub(crate) debug_info: Option<DebugInfo>,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("detail", &self.detail)
            .field("name", &self.name)
            .field("lineno", &self.lineno)
            .field("source", &self.source)
            .finish()?;

        // so this is a bit questionablem, but because of how commonly errors are just
        // unwrapped i think it's sensible to spit out the debug info following the
        // error struct dump.
        #[cfg(feature = "debug")]
        {
            if let Some(info) = self.debug_info() {
                writeln!(f)?;
                render_debug_info(f, self.line(), info)?;
                writeln!(f)?;
            }
        }

        Ok(())
    }
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
        #[cfg(feature = "debug")]
        {
            if f.alternate() {
                if let Some(info) = self.debug_info() {
                    render_debug_info(f, self.line(), info)?;
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
            source: None,
            #[cfg(feature = "debug")]
            debug_info: None,
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

    /// Returns the template debug information is available.
    ///
    /// The debug info snapshot is only embedded into the error if the debug
    /// mode is enabled on the environment
    /// ([`Environment::set_debug`](crate::Environment::set_debug)).
    #[cfg(feature = "debug")]
    #[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
    pub fn debug_info(&self) -> Option<&DebugInfo> {
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
            source: None,
            #[cfg(feature = "debug")]
            debug_info: None,
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

#[cfg(feature = "debug")]
mod debug_info {
    use super::*;
    use crate::value::Value;

    /// This is a snapshot of the debug information.
    #[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
    #[derive(Default)]
    pub struct DebugInfo {
        pub(crate) template_source: Option<String>,
        pub(crate) context: Option<Value>,
        pub(crate) referenced_names: Option<Vec<String>>,
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
        line: Option<usize>,
        info: &DebugInfo,
    ) -> fmt::Result {
        if let Some(source) = info.source() {
            writeln!(f)?;
            writeln!(f, "{:-^1$}", " Template Source ", 74).unwrap();
            let lines: Vec<_> = source.lines().enumerate().collect();
            let idx = line.unwrap_or(1) - 1;
            let skip = idx.saturating_sub(3);
            let pre = lines.iter().skip(skip).take(3.min(idx)).collect::<Vec<_>>();
            let post = lines.iter().skip(idx + 1).take(3).collect::<Vec<_>>();
            for (idx, line) in pre {
                writeln!(f, "{:>4} | {}", idx + 1, line).unwrap();
            }
            writeln!(f, "{:>4} > {}", idx + 1, lines[idx].1).unwrap();
            for (idx, line) in post {
                writeln!(f, "{:>4} | {}", idx + 1, line).unwrap();
            }
            write!(f, "{:-^1$}", "", 74).unwrap();
        }
        if let Some(ctx) = info.context() {
            if let Some(vars) = info.referenced_names() {
                writeln!(f)?;
                writeln!(f, "Referenced variables:")?;
                for var in vars {
                    write!(f, "  {:}: ", var)?;
                    match ctx.get_attr(var) {
                        Ok(val) => writeln!(f, "{:#?}", val)?,
                        Err(_) => writeln!(f, "undefined")?,
                    }
                }
            }
            write!(f, "{:-^1$}", "", 74).unwrap();
        }
        Ok(())
    }
}

#[cfg(feature = "debug")]
pub use self::debug_info::*;
