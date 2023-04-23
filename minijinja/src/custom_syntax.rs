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

#[test]
fn test_compile() {
    let syntax = Syntax {
        block_start: "{".into(),
        block_end: "}".into(),
        variable_start: "${".into(),
        variable_end: "}".into(),
        comment_start: "{*".into(),
        comment_end: "*}".into(),
    };

    let settings = syntax.compile().unwrap();

    let aho_corasick = settings.aho_corasick.as_ref().unwrap();

    let input = "{for x in range(3)}${x}{endfor}{* nothing *}";

    let mut matches = aho_corasick.find_iter(input);
    // '{'
    let statement_match = matches.next().unwrap();
    assert_eq!(statement_match.start(), 0);
    assert_eq!(statement_match.end(), 1);

    // '${'
    let var_match = matches.next().unwrap();
    assert_eq!(var_match.start(), 19);
    assert_eq!(var_match.end(), 21);

    // '{'
    let statemend_end_match = matches.next().unwrap();
    assert_eq!(statemend_end_match.start(), 23);
    assert_eq!(statemend_end_match.end(), 24);

    // '{*'
    let comment_match = matches.next().unwrap();
    assert_eq!(comment_match.start(), 31);
    assert_eq!(comment_match.end(), 33);

    assert!(matches.next().is_none());
}
