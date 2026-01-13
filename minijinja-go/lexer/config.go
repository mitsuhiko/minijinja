package lexer

// SyntaxConfig holds the delimiters and prefixes for template syntax.
//
// SyntaxConfig allows customization of the template syntax by changing
// the delimiters used for blocks, variables, and comments. This is useful
// when generating files where the default Jinja2 syntax would conflict
// with the target file format (e.g., LaTeX).
//
// # Default Syntax
//
// The default syntax uses Jinja2-style delimiters:
//   - Blocks: {% ... %}
//   - Variables: {{ ... }}
//   - Comments: {# ... #}
//
// # Custom Delimiters
//
// You can customize delimiters to avoid conflicts. For example, for LaTeX:
//
//	config := lexer.SyntaxConfig{
//	    BlockStart:   "\\BLOCK{",
//	    BlockEnd:     "}",
//	    VarStart:     "\\VAR{",
//	    VarEnd:       "}",
//	    CommentStart: "\\#{",
//	    CommentEnd:   "}",
//	}
//	env.SetSyntax(config)
//
// # Line Statements and Comments
//
// Line statements and comments provide an alternative syntax where blocks
// can be placed on their own line with a prefix. They must appear on their
// own line but can be prefixed with whitespace.
//
// Example with line syntax:
//
//	config := lexer.SyntaxConfig{
//	    BlockStart:          "{%",
//	    BlockEnd:            "%}",
//	    VarStart:            "{{",
//	    VarEnd:              "}}",
//	    CommentStart:        "{#",
//	    CommentEnd:          "#}",
//	    LineStatementPrefix: "#",
//	    LineCommentPrefix:   "##",
//	}
//
// Then in templates:
//
//	## This is a line comment
//	# for item in items
//	    <li>{{ item }}
//	# endfor
//
// See the syntax documentation for more details.
type SyntaxConfig struct {
	// BlockStart is the opening delimiter for blocks (default: "{%").
	BlockStart string

	// BlockEnd is the closing delimiter for blocks (default: "%}").
	BlockEnd string

	// VarStart is the opening delimiter for variables (default: "{{").
	VarStart string

	// VarEnd is the closing delimiter for variables (default: "}}").
	VarEnd string

	// CommentStart is the opening delimiter for comments (default: "{#").
	CommentStart string

	// CommentEnd is the closing delimiter for comments (default: "#}").
	CommentEnd string

	// LineStatementPrefix is the prefix for line statements (default: "").
	// When non-empty, lines starting with this prefix (after optional
	// whitespace) are treated as block statements.
	LineStatementPrefix string

	// LineCommentPrefix is the prefix for line comments (default: "").
	// When non-empty, content after this prefix is treated as a comment.
	LineCommentPrefix string
}

// DefaultSyntax returns the default Jinja2 syntax configuration.
//
// The default configuration uses standard Jinja2 delimiters:
//   - {% ... %} for blocks
//   - {{ ... }} for variables
//   - {# ... #} for comments
//   - No line statement or comment prefixes
//
// Example:
//
//	config := lexer.DefaultSyntax()
//	// Use with: env.SetSyntax(config)
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
//
// WhitespaceConfig controls how whitespace is handled in templates, including
// trailing newlines and whitespace around block tags. Proper whitespace
// configuration is important for generating well-formatted output.
//
// # Whitespace Control Modes
//
// MiniJinja shares the same behavior with Jinja2:
//   - By default, a single trailing newline is stripped and other whitespace
//     is returned unchanged.
//   - TrimBlocks removes the first newline after template tags.
//   - LstripBlocks strips leading whitespace before block tags on a line.
//   - KeepTrailingNewline preserves the final newline at the end of templates.
//
// # Example Without TrimBlocks/LstripBlocks
//
//	<div>
//	  {% if True %}
//	    yay
//	  {% endif %}
//	</div>
//
// Renders as:
//
//	<div>
//
//	    yay
//
//	</div>
//
// # Example With Both Enabled
//
// With TrimBlocks=true and LstripBlocks=true:
//
//	config := lexer.WhitespaceConfig{
//	    TrimBlocks:  true,
//	    LstripBlocks: true,
//	}
//	env.SetWhitespace(config)
//
// The same template renders as:
//
//	<div>
//	    yay
//	</div>
//
// # Manual Control
//
// Templates can manually control whitespace with + and - modifiers:
//
//	{% for item in items -%}  {{- item -}} {%- endfor %}
//
// See the syntax documentation for complete whitespace control details.
type WhitespaceConfig struct {
	// KeepTrailingNewline preserves the final newline at the end of templates.
	// Default: false (trailing newline is removed).
	KeepTrailingNewline bool

	// LstripBlocks strips leading whitespace before block tags.
	// Default: false.
	//
	// When enabled, tabs and spaces at the beginning of a line before a block
	// tag are removed. Nothing is stripped if there are other characters before
	// the block tag.
	LstripBlocks bool

	// TrimBlocks removes the first newline after block tags.
	// Default: false.
	//
	// When enabled, the first newline after a block tag is automatically
	// removed, similar to PHP's behavior.
	TrimBlocks bool
}

// DefaultWhitespace returns the default whitespace configuration.
//
// The default configuration preserves most whitespace but removes a single
// trailing newline:
//   - KeepTrailingNewline: false
//   - LstripBlocks: false
//   - TrimBlocks: false
//
// Example:
//
//	config := lexer.DefaultWhitespace()
//	// Use with: env.SetWhitespace(config)
func DefaultWhitespace() WhitespaceConfig {
	return WhitespaceConfig{
		KeepTrailingNewline: false,
		LstripBlocks:        false,
		TrimBlocks:          false,
	}
}
