use std::borrow::Cow;

/// The configuration for the environment and the parser.
/// This includes configurations to use custom delimiters
#[cfg(not(feature = "custom_delimiters"))]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Syntax {
    pub(crate) block_start: Cow<'static, str>,
    pub(crate) block_end: Cow<'static, str>,
    pub(crate) variable_start: Cow<'static, str>,
    pub(crate) variable_end: Cow<'static, str>,
    pub(crate) comment_start: Cow<'static, str>,
    pub(crate) comment_end: Cow<'static, str>,
}

/// The configuration for the environment and the parser.
/// This includes configurations to use custom delimiters
#[cfg(feature = "custom_delimiters")]
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
