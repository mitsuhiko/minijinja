use std::borrow::Cow;

#[cfg(feature = "custom_delimiters")]
use {
    crate::error::{Error, ErrorKind},
    aho_corasick::AhoCorasick,
};

/// The configuration for the environment and the parser.
/// This includes configurations to use custom delimiters
#[derive(Debug, Clone, Eq, PartialEq)]
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
            block_start: r#"{%"#.into(),
            block_end: r#"%}"#.into(),
            variable_start: r#"{{"#.into(),
            variable_end: r#"}}"#.into(),
            comment_start: r#"{#"#.into(),
            comment_end: r#"#}"#.into(),
        }
    }
}

#[cfg(feature = "custom_delimiters")]
impl Syntax {
    pub(crate) fn compile(self) -> Result<SyntaxConfig, Error> {
        ok!(self.check_delimiters());

        let patterns = [
            self.block_start.as_ref(),
            self.variable_start.as_ref(),
            self.comment_start.as_ref(),
        ];

        let aho_corasick =
            ok!(AhoCorasick::new(patterns).map_err(|_| ErrorKind::InvalidDelimiter.into()));
        Ok(SyntaxConfig {
            syntax: self,
            aho_corasick: Some(aho_corasick),
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

/// Internal configuration for the environment and the parser.
#[derive(Debug, Default)]
pub struct SyntaxConfig {
    pub(crate) syntax: Syntax,
    #[cfg(feature = "custom_delimiters")]
    pub(crate) aho_corasick: Option<aho_corasick::AhoCorasick>,
}
