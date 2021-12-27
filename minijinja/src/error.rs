use std::borrow::Cow;
use std::fmt;

use crate::value::Value;

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
            .finish()
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
        if f.alternate() {
            if let Some(source) = self.template_source() {
                writeln!(f)?;
                writeln!(f, "{:-^1$}", " Template Source ", 74).unwrap();
                let lines: Vec<_> = source.lines().enumerate().collect();
                let idx = self.line().unwrap_or(1) - 1;
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
            if let Some(ctx) = self.template_context() {
                if let Some(vars) = self
                    .debug_info
                    .as_ref()
                    .and_then(|x| x.referenced_names.as_ref())
                {
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

    /// Returns the template source if debug information is available.
    ///
    /// The template source is only embedded into the error if the debug
    /// mode is enabled on the environment
    /// ([`Environment::set_debug`](crate::Environment::set_debug)).  There
    /// are also situations where it's impossible to retrieve the source
    /// in which cases it can still be missing despite the debug mode being
    /// enabled.
    pub fn template_source(&self) -> Option<&str> {
        #[cfg(feature = "debug")]
        {
            self.debug_info
                .as_ref()
                .and_then(|x| x.template_source.as_ref())
                .map(|x| x.as_str())
        }
        #[cfg(not(feature = "debug"))]
        {
            None
        }
    }

    /// Returns the frozen template context if available.
    ///
    /// The engine will attempt to capture the context at the time when the
    /// error happened but in some cases it might not be entirely accurate.
    ///
    /// The template context is only embedded into the error if the debug
    /// mode is enabled on the environment
    /// ([`Environment::set_debug`](crate::Environment::set_debug)).
    pub fn template_context(&self) -> Option<Value> {
        #[cfg(feature = "debug")]
        {
            self.debug_info
                .as_ref()
                .and_then(|x| x.context.as_ref())
                .cloned()
        }
        #[cfg(not(feature = "debug"))]
        {
            None
        }
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
#[derive(Default)]
pub(crate) struct DebugInfo {
    pub(crate) template_source: Option<String>,
    pub(crate) context: Option<Value>,
    // for now this is internal
    pub(crate) referenced_names: Option<Vec<String>>,
}
