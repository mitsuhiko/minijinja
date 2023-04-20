use std::borrow::Cow;

#[cfg(feature = "custom_delimiters")]
use {
    crate::error::{Error, ErrorKind},
    aho_corasick::AhoCorasick,
};

/// The delimiter configuration for the environment and the parser.
/// This includes configurations to use custom delimiters
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(docsrs, doc(cfg(feature = "custom_delimiters")))]
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

impl Default for Syntax {
    fn default() -> Self {
        Syntax {
            block_start: "{%".into(),
            block_end: "%}".into(),
            variable_start: "{{".into(),
            variable_end: "}}".into(),
            comment_start: "{#".into(),
            comment_end: "#}".into(),
        }
    }
}

#[cfg(feature = "custom_delimiters")]
impl Syntax {
    /// Creates a new syntax configuration with custom delimiters.
    pub fn compile(self) -> Result<SyntaxConfig, Error> {
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

        let patterns = delimiter_order.map(|x| match x {
            StartMarker::Variable => &self.variable_start as &str,
            StartMarker::Block => &self.block_start as &str,
            StartMarker::Comment => &self.comment_start as &str,
        });

        let aho_corasick = ok!(AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostFirst)
            .build(patterns)
            .map_err(|_| ErrorKind::InvalidDelimiter.into()));
        Ok(SyntaxConfig {
            syntax: self,
            aho_corasick: Some(aho_corasick),
            start_delimiters_order: delimiter_order,
        })
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

#[derive(Debug, Copy, Clone)]
pub(crate) enum StartMarker {
    Variable,
    Block,
    Comment,
}

/// Internal configuration for the environment and the parser.
#[derive(Debug)]
pub struct SyntaxConfig {
    pub(crate) syntax: Syntax,
    pub(crate) start_delimiters_order: [StartMarker; 3],
    #[cfg(feature = "custom_delimiters")]
    pub(crate) aho_corasick: Option<aho_corasick::AhoCorasick>,
}

impl Default for SyntaxConfig {
    fn default() -> SyntaxConfig {
        SyntaxConfig {
            syntax: Syntax::default(),
            start_delimiters_order: [
                StartMarker::Variable,
                StartMarker::Block,
                StartMarker::Comment,
            ],
            #[cfg(feature = "custom_delimiters")]
            aho_corasick: None,
        }
    }
}

#[cfg(feature = "custom_delimiters")]
#[test]
fn test_compile() {
    use crate::Syntax;

    let syntax = Syntax {
        block_start: "{".into(),
        block_end: "}".into(),
        variable_start: "${".into(),
        variable_end: "}".into(),
        comment_start: "{*".into(),
        comment_end: "*}".into(),
    };

    let settings = syntax.compile().unwrap();

    let aho_corasick = settings.aho_corasick.unwrap();

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
    dbg!(&input.get(comment_match.start()..comment_match.end()));
    assert_eq!(comment_match.start(), 31);
    assert_eq!(comment_match.end(), 33);

    assert!(matches.next().is_none());
}
