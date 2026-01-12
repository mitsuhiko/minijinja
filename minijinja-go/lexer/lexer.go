package lexer

import (
	"fmt"
	"strings"
)

// Lexer tokenizes Jinja2 template source code.
type Lexer struct {
	input      string
	pos        int    // current position in input
	start      int    // start position of current token
	line       uint16 // current line (1-indexed)
	col        uint16 // current column (0-indexed)
	startLine  uint16
	startCol   uint16
	syntax     SyntaxConfig
	whitespace WhitespaceConfig

	// State tracking
	inBlock bool // true when inside {% %} or {{ }}
}

// New creates a new Lexer for the given input.
func New(input string, syntax SyntaxConfig, whitespace WhitespaceConfig) *Lexer {
	return &Lexer{
		input:      input,
		line:       1,
		col:        0,
		syntax:     syntax,
		whitespace: whitespace,
	}
}

// Tokenize returns all tokens from the input.
func Tokenize(input string, syntax SyntaxConfig, whitespace WhitespaceConfig) ([]Token, error) {
	l := New(input, syntax, whitespace)
	return l.All()
}

// All collects all tokens into a slice.
func (l *Lexer) All() ([]Token, error) {
	var tokens []Token
	for {
		tok, err := l.Next()
		if err != nil {
			return nil, err
		}
		if tok.Type == TokenEOF {
			break
		}
		tokens = append(tokens, tok)
	}
	return tokens, nil
}

// Next returns the next token.
func (l *Lexer) Next() (Token, error) {
	l.skipWhitespaceInBlock()
	l.markStart()

	if l.atEnd() {
		return l.makeToken(TokenEOF, ""), nil
	}

	if l.inBlock {
		return l.lexInsideBlock()
	}

	return l.lexTemplateData()
}

// lexTemplateData lexes raw template text until we hit a delimiter.
func (l *Lexer) lexTemplateData() (Token, error) {
	for !l.atEnd() {
		// Check for block/variable/comment start
		if l.matchesAt(l.syntax.VarStart) {
			if l.pos > l.start {
				// Emit template data before the delimiter
				return l.makeToken(TokenTemplateData, l.input[l.start:l.pos]), nil
			}
			l.advance(len(l.syntax.VarStart))
			l.inBlock = true
			return l.makeToken(TokenVariableStart, l.syntax.VarStart), nil
		}

		if l.matchesAt(l.syntax.BlockStart) {
			if l.pos > l.start {
				text := l.input[l.start:l.pos]
				// Handle lstrip_blocks: strip whitespace before block on same line
				if l.whitespace.LstripBlocks {
					text = l.lstripBlockText(text)
				}
				if text != "" {
					return l.makeToken(TokenTemplateData, text), nil
				}
				l.markStart()
			}
			l.advance(len(l.syntax.BlockStart))
			l.inBlock = true
			return l.makeToken(TokenBlockStart, l.syntax.BlockStart), nil
		}

		if l.matchesAt(l.syntax.CommentStart) {
			if l.pos > l.start {
				text := l.input[l.start:l.pos]
				if l.whitespace.LstripBlocks {
					text = l.lstripBlockText(text)
				}
				if text != "" {
					return l.makeToken(TokenTemplateData, text), nil
				}
				l.markStart()
			}
			// Skip entire comment
			l.advance(len(l.syntax.CommentStart))
			for !l.atEnd() && !l.matchesAt(l.syntax.CommentEnd) {
				l.advanceOne()
			}
			if l.matchesAt(l.syntax.CommentEnd) {
				l.advance(len(l.syntax.CommentEnd))
			}
			// Handle trim_blocks after comment
			if l.whitespace.TrimBlocks && l.peekChar() == '\n' {
				l.advanceOne()
			}
			l.markStart()
			continue
		}

		l.advanceOne()
	}

	// Return any remaining template data
	if l.pos > l.start {
		text := l.input[l.start:l.pos]
		// Handle keep_trailing_newline
		if !l.whitespace.KeepTrailingNewline && strings.HasSuffix(text, "\n") {
			text = strings.TrimSuffix(text, "\n")
			if text == "" {
				return l.makeToken(TokenEOF, ""), nil
			}
		}
		return l.makeToken(TokenTemplateData, text), nil
	}

	return l.makeToken(TokenEOF, ""), nil
}

// lexInsideBlock lexes tokens inside {% %} or {{ }}.
func (l *Lexer) lexInsideBlock() (Token, error) {
	l.skipWhitespaceInBlock()
	l.markStart()

	if l.atEnd() {
		return Token{}, fmt.Errorf("unexpected end of input inside block")
	}

	// Check for block end
	if l.matchesAt(l.syntax.VarEnd) {
		l.advance(len(l.syntax.VarEnd))
		l.inBlock = false
		return l.makeToken(TokenVariableEnd, l.syntax.VarEnd), nil
	}

	if l.matchesAt(l.syntax.BlockEnd) {
		l.advance(len(l.syntax.BlockEnd))
		l.inBlock = false
		// Handle trim_blocks
		if l.whitespace.TrimBlocks && l.peekChar() == '\n' {
			l.advanceOne()
		}
		return l.makeToken(TokenBlockEnd, l.syntax.BlockEnd), nil
	}

	ch := l.peekChar()

	// Operators (multi-char first)
	if l.matchesAt("//") {
		l.advance(2)
		return l.makeToken(TokenFloorDiv, "//"), nil
	}
	if l.matchesAt("**") {
		l.advance(2)
		return l.makeToken(TokenPow, "**"), nil
	}
	if l.matchesAt("==") {
		l.advance(2)
		return l.makeToken(TokenEq, "=="), nil
	}
	if l.matchesAt("!=") {
		l.advance(2)
		return l.makeToken(TokenNe, "!="), nil
	}
	if l.matchesAt("<=") {
		l.advance(2)
		return l.makeToken(TokenLe, "<="), nil
	}
	if l.matchesAt(">=") {
		l.advance(2)
		return l.makeToken(TokenGe, ">="), nil
	}

	// Single char operators/punctuation
	switch ch {
	case '+':
		l.advanceOne()
		return l.makeToken(TokenPlus, "+"), nil
	case '-':
		l.advanceOne()
		return l.makeToken(TokenMinus, "-"), nil
	case '*':
		l.advanceOne()
		return l.makeToken(TokenMul, "*"), nil
	case '/':
		l.advanceOne()
		return l.makeToken(TokenDiv, "/"), nil
	case '%':
		l.advanceOne()
		return l.makeToken(TokenMod, "%"), nil
	case '~':
		l.advanceOne()
		return l.makeToken(TokenTilde, "~"), nil
	case '<':
		l.advanceOne()
		return l.makeToken(TokenLt, "<"), nil
	case '>':
		l.advanceOne()
		return l.makeToken(TokenGt, ">"), nil
	case '=':
		l.advanceOne()
		return l.makeToken(TokenAssign, "="), nil
	case '.':
		l.advanceOne()
		return l.makeToken(TokenDot, "."), nil
	case ',':
		l.advanceOne()
		return l.makeToken(TokenComma, ","), nil
	case ':':
		l.advanceOne()
		return l.makeToken(TokenColon, ":"), nil
	case '|':
		l.advanceOne()
		return l.makeToken(TokenPipe, "|"), nil
	case '(':
		l.advanceOne()
		return l.makeToken(TokenParenOpen, "("), nil
	case ')':
		l.advanceOne()
		return l.makeToken(TokenParenClose, ")"), nil
	case '[':
		l.advanceOne()
		return l.makeToken(TokenBracketOpen, "["), nil
	case ']':
		l.advanceOne()
		return l.makeToken(TokenBracketClose, "]"), nil
	case '{':
		l.advanceOne()
		return l.makeToken(TokenBraceOpen, "{"), nil
	case '}':
		l.advanceOne()
		return l.makeToken(TokenBraceClose, "}"), nil
	}

	// String literals
	if ch == '"' || ch == '\'' {
		return l.lexString(ch)
	}

	// Numbers
	if isDigit(ch) {
		return l.lexNumber()
	}

	// Identifiers and keywords
	if isIdentStart(ch) {
		return l.lexIdent()
	}

	return Token{}, fmt.Errorf("unexpected character %q at line %d, col %d", ch, l.line, l.col)
}

// lexString lexes a string literal.
func (l *Lexer) lexString(quote byte) (Token, error) {
	l.advanceOne() // skip opening quote

	var sb strings.Builder
	for !l.atEnd() {
		ch := l.peekChar()
		if ch == quote {
			l.advanceOne()
			return l.makeToken(TokenString, sb.String()), nil
		}
		if ch == '\\' {
			l.advanceOne()
			if l.atEnd() {
				return Token{}, fmt.Errorf("unexpected end of string")
			}
			escaped := l.peekChar()
			l.advanceOne()
			switch escaped {
			case 'n':
				sb.WriteByte('\n')
			case 't':
				sb.WriteByte('\t')
			case 'r':
				sb.WriteByte('\r')
			case '\\':
				sb.WriteByte('\\')
			case '\'':
				sb.WriteByte('\'')
			case '"':
				sb.WriteByte('"')
			default:
				sb.WriteByte('\\')
				sb.WriteByte(escaped)
			}
		} else {
			sb.WriteByte(ch)
			l.advanceOne()
		}
	}

	return Token{}, fmt.Errorf("unterminated string")
}

// lexNumber lexes an integer or float literal.
func (l *Lexer) lexNumber() (Token, error) {
	isFloat := false

	for !l.atEnd() && isDigit(l.peekChar()) {
		l.advanceOne()
	}

	// Check for decimal point
	if l.peekChar() == '.' && l.pos+1 < len(l.input) && isDigit(l.input[l.pos+1]) {
		isFloat = true
		l.advanceOne() // skip '.'
		for !l.atEnd() && isDigit(l.peekChar()) {
			l.advanceOne()
		}
	}

	// Check for exponent
	if l.peekChar() == 'e' || l.peekChar() == 'E' {
		isFloat = true
		l.advanceOne()
		if l.peekChar() == '+' || l.peekChar() == '-' {
			l.advanceOne()
		}
		for !l.atEnd() && isDigit(l.peekChar()) {
			l.advanceOne()
		}
	}

	value := l.input[l.start:l.pos]
	if isFloat {
		return l.makeToken(TokenFloat, value), nil
	}
	return l.makeToken(TokenInteger, value), nil
}

// lexIdent lexes an identifier.
// Note: Keywords are NOT identified at lexer level - they're just identifiers.
// The parser is responsible for recognizing keywords.
func (l *Lexer) lexIdent() (Token, error) {
	for !l.atEnd() && isIdentPart(l.peekChar()) {
		l.advanceOne()
	}

	value := l.input[l.start:l.pos]
	return l.makeToken(TokenIdent, value), nil
}

// keywords maps keyword strings to token types.
var keywords = map[string]TokenType{
	"true":          TokenTrue,
	"True":          TokenTrue,
	"false":         TokenFalse,
	"False":         TokenFalse,
	"none":          TokenNone,
	"None":          TokenNone,
	"and":           TokenAnd,
	"or":            TokenOr,
	"not":           TokenNot,
	"in":            TokenIn,
	"is":            TokenIs,
	"if":            TokenIf,
	"else":          TokenElse,
	"elif":          TokenElif,
	"endif":         TokenEndif,
	"for":           TokenFor,
	"endfor":        TokenEndfor,
	"block":         TokenBlock,
	"endblock":      TokenEndblock,
	"extends":       TokenExtends,
	"include":       TokenInclude,
	"import":        TokenImport,
	"from":          TokenFrom,
	"as":            TokenAs,
	"with":          TokenWith,
	"endwith":       TokenEndwith,
	"set":           TokenSet,
	"endset":        TokenEndset,
	"macro":         TokenMacro,
	"endmacro":      TokenEndmacro,
	"call":          TokenCall,
	"endcall":       TokenEndcall,
	"filter":        TokenFilter,
	"endfilter":     TokenEndfilter,
	"raw":           TokenRaw,
	"endraw":        TokenEndraw,
	"autoescape":    TokenAutoescape,
	"endautoescape": TokenEndautoescape,
	"do":            TokenDo,
	"continue":      TokenContinue,
	"break":         TokenBreak,
	"recursive":     TokenRecursive,
	"ignore":        TokenIgnoreMissing, // part of "ignore missing"
}

func lookupKeyword(ident string) TokenType {
	if tok, ok := keywords[ident]; ok {
		return tok
	}
	return TokenIdent
}

// Helper methods

func (l *Lexer) atEnd() bool {
	return l.pos >= len(l.input)
}

func (l *Lexer) peekChar() byte {
	if l.atEnd() {
		return 0
	}
	return l.input[l.pos]
}

func (l *Lexer) advanceOne() {
	if l.atEnd() {
		return
	}
	if l.input[l.pos] == '\n' {
		l.line++
		l.col = 0
	} else {
		if l.col < 65535 {
			l.col++
		}
	}
	l.pos++
}

func (l *Lexer) advance(n int) {
	for i := 0; i < n; i++ {
		l.advanceOne()
	}
}

func (l *Lexer) markStart() {
	l.start = l.pos
	l.startLine = l.line
	l.startCol = l.col
}

func (l *Lexer) matchesAt(s string) bool {
	return strings.HasPrefix(l.input[l.pos:], s)
}

func (l *Lexer) makeToken(typ TokenType, value string) Token {
	return Token{
		Type:  typ,
		Value: value,
		Span: Span{
			StartLine:   l.startLine,
			StartCol:    l.startCol,
			StartOffset: uint32(l.start),
			EndLine:     l.line,
			EndCol:      l.col,
			EndOffset:   uint32(l.pos),
		},
	}
}

func (l *Lexer) skipWhitespaceInBlock() {
	if !l.inBlock {
		return
	}
	for !l.atEnd() {
		ch := l.peekChar()
		if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
			l.advanceOne()
		} else {
			break
		}
	}
}

func (l *Lexer) lstripBlockText(text string) string {
	// Find last newline, strip whitespace after it
	idx := strings.LastIndex(text, "\n")
	if idx == -1 {
		// No newline, check if entire text is whitespace
		if strings.TrimSpace(text) == "" {
			return ""
		}
		return text
	}
	// Keep everything up to and including the newline, strip trailing whitespace
	prefix := text[:idx+1]
	suffix := text[idx+1:]
	if strings.TrimSpace(suffix) == "" {
		return prefix
	}
	return text
}

func isDigit(ch byte) bool {
	return ch >= '0' && ch <= '9'
}

func isIdentStart(ch byte) bool {
	return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch == '_'
}

func isIdentPart(ch byte) bool {
	return isIdentStart(ch) || isDigit(ch)
}
