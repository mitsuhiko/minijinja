// Package lexer provides tokenization for Jinja2 templates.
package lexer

import (
	"fmt"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/syntax"
)

// TokenType represents the type of a token.
type TokenType int

const (
	// Template data (raw text between tags)
	TokenTemplateData TokenType = iota

	// Delimiters
	TokenVariableStart // {{
	TokenVariableEnd   // }}
	TokenBlockStart    // {%
	TokenBlockEnd      // %}
	TokenCommentStart  // {#
	TokenCommentEnd    // #}

	// Literals
	TokenIdent   // identifier
	TokenString  // "string" or 'string'
	TokenInteger // 123 (fits in u64)
	TokenInt128  // big integers (> u64)
	TokenFloat   // 123.45

	// Operators
	TokenPlus     // +
	TokenMinus    // -
	TokenMul      // *
	TokenDiv      // /
	TokenFloorDiv // //
	TokenMod      // %
	TokenPow      // **
	TokenTilde    // ~

	// Comparison
	TokenEq // ==
	TokenNe // !=
	TokenLt // <
	TokenLe // <=
	TokenGt // >
	TokenGe // >=

	// Assignment
	TokenAssign // =

	// Punctuation
	TokenDot          // .
	TokenComma        // ,
	TokenColon        // :
	TokenPipe         // |
	TokenParenOpen    // (
	TokenParenClose   // )
	TokenBracketOpen  // [
	TokenBracketClose // ]
	TokenBraceOpen    // {
	TokenBraceClose   // }

	// Keywords (detected from identifiers)
	TokenTrue
	TokenFalse
	TokenNone
	TokenAnd
	TokenOr
	TokenNot
	TokenIn
	TokenIs
	TokenIf
	TokenElse
	TokenElif
	TokenEndif
	TokenFor
	TokenEndfor
	TokenBlock
	TokenEndblock
	TokenExtends
	TokenInclude
	TokenImport
	TokenFrom
	TokenAs
	TokenWith
	TokenEndwith
	TokenSet
	TokenEndset
	TokenMacro
	TokenEndmacro
	TokenCall
	TokenEndcall
	TokenFilter
	TokenEndfilter
	TokenRaw
	TokenEndraw
	TokenAutoescape
	TokenEndautoescape
	TokenDo
	TokenContinue
	TokenBreak
	TokenRecursive
	TokenIgnoreMissing

	// Special
	TokenEOF
	TokenError
)

// Token represents a single token from the lexer.
type Token struct {
	Type  TokenType
	Value string // The token value (for idents, strings, numbers, template data)
	Span  Span   // Source location
}

// Span represents a location range in source code.
type Span = syntax.Span

// String returns a debug representation of the token.
func (t Token) String() string {
	return fmt.Sprintf("%s(%q)", t.Type, t.Value)
}

// tokenTypeNames maps token types to their string representations.
var tokenTypeNames = map[TokenType]string{
	TokenTemplateData:  "TemplateData",
	TokenVariableStart: "VariableStart",
	TokenVariableEnd:   "VariableEnd",
	TokenBlockStart:    "BlockStart",
	TokenBlockEnd:      "BlockEnd",
	TokenCommentStart:  "CommentStart",
	TokenCommentEnd:    "CommentEnd",
	TokenIdent:         "Ident",
	TokenString:        "String",
	TokenInteger:       "Int",
	TokenInt128:        "Int128",
	TokenFloat:         "Float",
	TokenPlus:          "Plus",
	TokenMinus:         "Minus",
	TokenMul:           "Mul",
	TokenDiv:           "Div",
	TokenFloorDiv:      "FloorDiv",
	TokenMod:           "Mod",
	TokenPow:           "Pow",
	TokenTilde:         "Tilde",
	TokenEq:            "Eq",
	TokenNe:            "Ne",
	TokenLt:            "Lt",
	TokenLe:            "Le",
	TokenGt:            "Gt",
	TokenGe:            "Ge",
	TokenAssign:        "Assign",
	TokenDot:           "Dot",
	TokenComma:         "Comma",
	TokenColon:         "Colon",
	TokenPipe:          "Pipe",
	TokenParenOpen:     "ParenOpen",
	TokenParenClose:    "ParenClose",
	TokenBracketOpen:   "BracketOpen",
	TokenBracketClose:  "BracketClose",
	TokenBraceOpen:     "BraceOpen",
	TokenBraceClose:    "BraceClose",
	TokenTrue:          "True",
	TokenFalse:         "False",
	TokenNone:          "None",
	TokenAnd:           "And",
	TokenOr:            "Or",
	TokenNot:           "Not",
	TokenIn:            "In",
	TokenIs:            "Is",
	TokenIf:            "If",
	TokenElse:          "Else",
	TokenElif:          "Elif",
	TokenEndif:         "Endif",
	TokenFor:           "For",
	TokenEndfor:        "Endfor",
	TokenBlock:         "Block",
	TokenEndblock:      "Endblock",
	TokenExtends:       "Extends",
	TokenInclude:       "Include",
	TokenImport:        "Import",
	TokenFrom:          "From",
	TokenAs:            "As",
	TokenWith:          "With",
	TokenEndwith:       "Endwith",
	TokenSet:           "Set",
	TokenEndset:        "Endset",
	TokenMacro:         "Macro",
	TokenEndmacro:      "Endmacro",
	TokenCall:          "Call",
	TokenEndcall:       "Endcall",
	TokenFilter:        "Filter",
	TokenEndfilter:     "Endfilter",
	TokenRaw:           "Raw",
	TokenEndraw:        "Endraw",
	TokenAutoescape:    "Autoescape",
	TokenEndautoescape: "Endautoescape",
	TokenDo:            "Do",
	TokenContinue:      "Continue",
	TokenBreak:         "Break",
	TokenRecursive:     "Recursive",
	TokenIgnoreMissing: "IgnoreMissing",
	TokenEOF:           "EOF",
	TokenError:         "Error",
}

func (t TokenType) String() string {
	if name, ok := tokenTypeNames[t]; ok {
		return name
	}
	return fmt.Sprintf("TokenType(%d)", t)
}

// FormatForSnapshot formats a token the way Insta snapshots expect.
func (t Token) FormatForSnapshot(source string) string {
	// Get the source substring for this token
	tokenSource := source[t.Span.StartOffset:t.Span.EndOffset]

	// Format like Rust's Debug output for tokens
	// Note: Rust uses Str/Int, not String/Integer
	switch t.Type {
	case TokenTemplateData:
		return fmt.Sprintf("TemplateData(%q)\n  %q", t.Value, tokenSource)
	case TokenIdent:
		return fmt.Sprintf("Ident(%q)\n  %q", t.Value, tokenSource)
	case TokenString:
		return fmt.Sprintf("Str(%q)\n  %q", t.Value, tokenSource)
	case TokenInteger:
		return fmt.Sprintf("Int(%s)\n  %q", t.Value, tokenSource)
	case TokenInt128:
		return fmt.Sprintf("Int128(%s)\n  %q", t.Value, tokenSource)
	case TokenFloat:
		return fmt.Sprintf("Float(%s)\n  %q", t.Value, tokenSource)
	default:
		return fmt.Sprintf("%s\n  %q", t.Type, tokenSource)
	}
}
