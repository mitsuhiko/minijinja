use aho_corasick::AhoCorasick;
use std::borrow::Cow;
use std::sync::Arc;

use crate::compiler::lexer::StartMarker;
use crate::error::{Error, ErrorKind};

/// The delimiter configuration for the environment and the parser.
///
/// MiniJinja allows you to override the syntax configuration for
/// templates by setting different delimiters.  The end markers can
/// be shared, but the start markers need to be distinct.  It would
/// thus not be valid to configure `{{` to be the marker for both
/// variables and blocks.
///
/// ```
/// # use minijinja::{Environment, Syntax};
/// let mut environment = Environment::new();
/// environment.set_syntax(Syntax {
///     block_start: "\\BLOCK{".into(),
///     block_end: "}".into(),
///     variable_start: "\\VAR{".into(),
///     variable_end: "}".into(),
///     comment_start: "\\#{".into(),
///     comment_end: "}".into(),
/// }).unwrap();
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(docsrs, doc(cfg(feature = "custom_syntax")))]
pub struct Syntax {
    /// The start of a block. By default it is `{%`.
    pub block_start: Cow<'static, str>,
    /// The end of a block. By default it is `%}`.
    pub block_end: Cow<'static, str>,
    /// The start of a variable. By default it is `{{`.
    pub variable_start: Cow<'static, str>,
    /// The end of a variable. By default it is `}}`.
    pub variable_end: Cow<'static, str>,
    /// The start of a comment. By default it is `{#`.
    pub comment_start: Cow<'static, str>,
    /// The end of a comment. By default it is `#}`.
    pub comment_end: Cow<'static, str>,
}

const DEFAULT_SYNTAX: Syntax = Syntax {
    block_start: Cow::Borrowed("{%"),
    block_end: Cow::Borrowed("%}"),
    variable_start: Cow::Borrowed("{{"),
    variable_end: Cow::Borrowed("}}"),
    comment_start: Cow::Borrowed("{#"),
    comment_end: Cow::Borrowed("#}"),
};

impl Default for Syntax {
    fn default() -> Self {
        DEFAULT_SYNTAX
    }
}

impl Syntax {
    /// Creates a new syntax configuration with custom delimiters.
    pub(crate) fn compile(self) -> Result<SyntaxConfig, Error> {
        if self == DEFAULT_SYNTAX {
            return Ok(SyntaxConfig::default());
        }

        ok!(self.check_delimiters());

        let mut delimiter_order = [
            StartMarker::Variable,
            StartMarker::Block,
            StartMarker::Comment,
        ];
        delimiter_order.sort_by_key(|marker| {
            std::cmp::Reverse(match marker {
                StartMarker::Variable => self.variable_start.len(),
                StartMarker::Block => self.block_start.len(),
                StartMarker::Comment => self.comment_start.len(),
            })
        });

        let aho_corasick = ok!(AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build([
                &self.variable_start as &str,
                &self.block_start as &str,
                &self.comment_start as &str,
            ])
            .map_err(|_| ErrorKind::InvalidDelimiter.into()));
        Ok(Arc::new(SyntaxConfigInternal {
            syntax: self,
            aho_corasick: Some(aho_corasick),
            start_delimiters_order: delimiter_order,
        }))
    }

    /// block, variable and comment start strings must be different
    fn check_delimiters(&self) -> Result<(), Error> {
        if self.block_start != self.variable_start
            && self.block_start != self.comment_start
            && self.variable_start != self.comment_start
        {
            Ok(())
        } else {
            Err(ErrorKind::InvalidDelimiter.into())
        }
    }
}

/// Internal configuration for the environment and the parser.
#[derive(Debug)]
pub struct SyntaxConfigInternal {
    pub(crate) syntax: Syntax,
    pub(crate) start_delimiters_order: [StartMarker; 3],
    pub(crate) aho_corasick: Option<aho_corasick::AhoCorasick>,
}

/// Configurable syntax config
pub type SyntaxConfig = Arc<SyntaxConfigInternal>;

impl Default for SyntaxConfigInternal {
    fn default() -> Self {
        SyntaxConfigInternal {
            syntax: Syntax::default(),
            start_delimiters_order: [
                StartMarker::Variable,
                StartMarker::Block,
                StartMarker::Comment,
            ],
            aho_corasick: None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct Delims {
    block_start: Cow<'static, str>,
    block_end: Cow<'static, str>,
    variable_start: Cow<'static, str>,
    variable_end: Cow<'static, str>,
    comment_start: Cow<'static, str>,
    comment_end: Cow<'static, str>,
}

const DEFAULT_DELIMS: Delims = Delims {
    block_start: Cow::Borrowed("{%"),
    block_end: Cow::Borrowed("%}"),
    variable_start: Cow::Borrowed("{{"),
    variable_end: Cow::Borrowed("}}"),
    comment_start: Cow::Borrowed("{#"),
    comment_end: Cow::Borrowed("#}"),
};

/// Builder helper to reconfigure the syntax.
#[derive(Debug)]
pub struct SyntaxBuilder {
    delims: Arc<Delims>,
}

impl SyntaxBuilder {
    /// Sets the block start and end delimiters.
    pub fn block_delimiters<S, E>(&mut self, s: S, e: E) -> &mut Self
    where
        S: Into<Cow<'static, str>>,
        E: Into<Cow<'static, str>>,
    {
        let delims = Arc::make_mut(&mut self.delims);
        delims.block_start = s.into();
        delims.block_end = e.into();
        self
    }

    /// Sets the variable start and end delimiters.
    pub fn variable_delimiters<S, E>(&mut self, s: S, e: E) -> &mut Self
    where
        S: Into<Cow<'static, str>>,
        E: Into<Cow<'static, str>>,
    {
        let delims = Arc::make_mut(&mut self.delims);
        delims.variable_start = s.into();
        delims.variable_end = e.into();
        self
    }

    /// Sets the comment start and end delimiters.
    pub fn comment_delimiters<S, E>(&mut self, s: S, e: E) -> &mut Self
    where
        S: Into<Cow<'static, str>>,
        E: Into<Cow<'static, str>>,
    {
        let delims = Arc::make_mut(&mut self.delims);
        delims.comment_start = s.into();
        delims.comment_end = e.into();
        self
    }

    /// Builds the final syntax config.
    pub fn build(&self) -> Result<SyntaxConfig2, Error> {
        let delims = self.delims.clone();
        if *delims == DEFAULT_DELIMS {
            return Ok(SyntaxConfig2::default());
        } else if delims.block_start == delims.variable_start
            || delims.block_start == delims.comment_start
            || delims.variable_start == delims.comment_start
        {
            return Err(ErrorKind::InvalidDelimiter.into());
        }
        let mut start_delimiters_order = [
            StartMarker::Variable,
            StartMarker::Block,
            StartMarker::Comment,
        ];
        start_delimiters_order.sort_by_key(|marker| {
            std::cmp::Reverse(match marker {
                StartMarker::Variable => delims.variable_start.len(),
                StartMarker::Block => delims.block_start.len(),
                StartMarker::Comment => delims.comment_start.len(),
            })
        });
        let aho_corasick = ok!(AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build([
                &delims.variable_start as &str,
                &delims.block_start as &str,
                &delims.comment_start as &str,
            ])
            .map_err(|_| ErrorKind::InvalidDelimiter.into()));
        Ok(SyntaxConfig2 {
            delims,
            start_delimiters_order,
            aho_corasick: Some(aho_corasick),
        })
    }
}

struct SyntaxConfig2 {
    pub(crate) delims: Arc<Delims>,
    pub(crate) start_delimiters_order: [StartMarker; 3],
    pub(crate) aho_corasick: Option<aho_corasick::AhoCorasick>,
}

impl Default for SyntaxConfig2 {
    fn default() -> Self {
        Self {
            delims: Arc::new(DEFAULT_DELIMS),
            start_delimiters_order: [
                StartMarker::Variable,
                StartMarker::Block,
                StartMarker::Comment,
            ],
            aho_corasick: None,
        }
    }
}

impl SyntaxConfig2 {
    /// Creates a syntax builder.
    pub fn builder() -> SyntaxBuilder {
        SyntaxBuilder {
            delims: Arc::new(DEFAULT_DELIMS),
        }
    }

    /// Returns the block delimiters.
    pub fn block_delimiters(&self) -> (&str, &str) {
        (&self.delims.block_start, &self.delims.block_end)
    }

    /// Returns the variable delimiters.
    pub fn variable_delimiters(&self) -> (&str, &str) {
        (&self.delims.variable_start, &self.delims.variable_end)
    }

    /// Returns the comment delimiters.
    pub fn comment_delimiters(&self) -> (&str, &str) {
        (&self.delims.comment_start, &self.delims.comment_end)
    }
}
