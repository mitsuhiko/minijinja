use std::borrow::Cow;

/// The configuration for the environment and the parser.
/// This includes configurations to use custom delimiters
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Syntax {
    pub(crate) block_start: Cow<'static, str>,
    pub(crate) block_end: Cow<'static, str>,
    pub(crate) variable_start: Cow<'static, str>,
    pub(crate) variable_end: Cow<'static, str>,
    pub(crate) comment_start: Cow<'static, str>,
    pub(crate) comment_end: Cow<'static, str>,
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

impl Syntax {
    /// Creates a new settings object with custom delimiters.
    #[cfg(feature = "custom_delimiters")]
    pub fn new<T>(
        block_start: T,
        block_end: T,
        variable_start: T,
        variable_end: T,
        comment_start: T,
        comment_end: T,
    ) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        Syntax {
            block_start: block_start.into(),
            block_end: block_end.into(),
            variable_start: variable_start.into(),
            variable_end: variable_end.into(),
            comment_start: comment_start.into(),
            comment_end: comment_end.into(),
        }
    }

    /// Sets the block delimiters.
    #[cfg(feature = "custom_delimiters")]
    pub fn set_block_delimiters<T>(&mut self, start: T, end: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.block_start = start.into();
        self.block_end = end.into();
    }

    /// Sets the variable delimiters.
    #[cfg(feature = "custom_delimiters")]
    pub fn set_variable_delimiters<T>(&mut self, start: T, end: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.variable_start = start.into();
        self.variable_end = end.into();
    }

    /// Sets the comment delimiters.
    #[cfg(feature = "custom_delimiters")]
    pub fn set_comment_delimiters<T>(&mut self, start: T, end: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.comment_start = start.into();
        self.comment_end = end.into();
    }
}
