package lexer

// SyntaxConfig holds the delimiters and prefixes for template syntax.
type SyntaxConfig struct {
	BlockStart   string
	BlockEnd     string
	VarStart     string
	VarEnd       string
	CommentStart string
	CommentEnd   string

	LineStatementPrefix string
	LineCommentPrefix   string
}

// DefaultSyntax returns the default Jinja2 syntax configuration.
func DefaultSyntax() SyntaxConfig {
	return SyntaxConfig{
		BlockStart:   "{%",
		BlockEnd:     "%}",
		VarStart:     "{{",
		VarEnd:       "}}",
		CommentStart: "{#",
		CommentEnd:   "#}",
	}
}

// WhitespaceConfig holds whitespace handling configuration.
type WhitespaceConfig struct {
	KeepTrailingNewline bool
	LstripBlocks        bool
	TrimBlocks          bool
}

// DefaultWhitespace returns the default whitespace configuration.
func DefaultWhitespace() WhitespaceConfig {
	return WhitespaceConfig{
		KeepTrailingNewline: false,
		LstripBlocks:        false,
		TrimBlocks:          false,
	}
}
