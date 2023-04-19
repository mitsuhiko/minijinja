/// The configuration for the environment and the parser.
/// This includes configurations to use custom delimiters
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Syntax {
    pub(crate) block_start: &'static str,
    pub(crate) block_end: &'static str,
    pub(crate) variable_start: &'static str,
    pub(crate) variable_end: &'static str,
    pub(crate) comment_start: &'static str,
    pub(crate) comment_end: &'static str,
}

impl Default for Syntax {
    fn default() -> Self {
        Syntax {
            block_start: r#"{%"#,
            block_end: r#"%}"#,
            variable_start: r#"{{"#,
            variable_end: r#"}}"#,
            comment_start: r#"{#"#,
            comment_end: r#"#}"#,
        }
    }
}

impl Syntax {
    #[cfg(feature = "custom_delimiters")]
    /// Creates a new settings object with custom delimiters.
    pub fn new(
        block_start: &'static str,
        block_end: &'static str,
        variable_start: &'static str,
        variable_end: &'static str,
        comment_start: &'static str,
        comment_end: &'static str,
    ) -> Self {
        Syntax {
            block_start,
            block_end,
            variable_start,
            variable_end,
            comment_start,
            comment_end,
        }
    }

    #[cfg(feature = "custom_delimiters")]
    /// Sets the block delimiters.
    pub fn set_block_delimiters(&mut self, start: &'static str, end: &'static str) {
        self.block_start = start;
        self.block_end = end;
    }

    #[cfg(feature = "custom_delimiters")]
    /// Sets the variable delimiters.
    pub fn set_variable_delimiters(&mut self, start: &'static str, end: &'static str) {
        self.variable_start = start;
        self.variable_end = end;
    }

    #[cfg(feature = "custom_delimiters")]
    /// Sets the comment delimiters.
    pub fn set_comment_delimiters(&mut self, start: &'static str, end: &'static str) {
        self.comment_start = start;
        self.comment_end = end;
    }
}
