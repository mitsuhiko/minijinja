package lexer

import (
	"fmt"
	"math/big"
	"strconv"
	"strings"
)

// Lexer tokenizes Jinja2 template source code.
type Lexer struct {
	source     string // original source (possibly with trailing newline stripped)
	pos        int    // current position in source
	start      int    // start position of current token
	line       uint16 // current line (1-indexed)
	col        uint16 // current column (0-indexed at line start)
	startLine  uint16
	startCol   uint16
	syntax     SyntaxConfig
	whitespace WhitespaceConfig

	// State tracking
	stack                   []lexerState
	trimLeadingWhitespace   bool
	pendingStartMarker      *pendingMarker
	parenBalance            int
}

type lexerState int

const (
	stateTemplate lexerState = iota
	stateVariable
	stateBlock
	stateLineStatement
)

type pendingMarker struct {
	marker      startMarker
	length      int
	prefixStart int
}

type startMarker int

const (
	markerVariable startMarker = iota
	markerBlock
	markerComment
	markerLineStatement
	markerLineComment
)

type whitespaceMode int

const (
	wsDefault whitespaceMode = iota
	wsPreserve // +
	wsRemove   // -
)

func whitespaceFromByte(b byte) whitespaceMode {
	switch b {
	case '-':
		return wsRemove
	case '+':
		return wsPreserve
	default:
		return wsDefault
	}
}

// New creates a new Lexer for the given input.
func New(input string, syntax SyntaxConfig, whitespace WhitespaceConfig) *Lexer {
	source := input
	// Strip trailing newline if not keeping it (like Rust does)
	if !whitespace.KeepTrailingNewline {
		if strings.HasSuffix(source, "\n") {
			source = source[:len(source)-1]
		}
		if strings.HasSuffix(source, "\r") {
			source = source[:len(source)-1]
		}
	}

	return &Lexer{
		source:     source,
		line:       1,
		col:        0,
		syntax:     syntax,
		whitespace: whitespace,
		stack:      []lexerState{stateTemplate},
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
		if tok == nil {
			break
		}
		tokens = append(tokens, *tok)
	}
	return tokens, nil
}

// Next returns the next token, or nil at end of input.
func (l *Lexer) Next() (*Token, error) {
	for {
		if l.atEnd() {
			// Line statements close at end of file if still open
			if l.currentState() == stateLineStatement {
				l.popState()
				l.markStart()
				tok := l.makeToken(TokenBlockEnd, "")
				return &tok, nil
			}
			return nil, nil
		}

		state := l.currentState()
		var tok *Token
		var err error
		var cont bool

		switch state {
		case stateTemplate:
			tok, cont, err = l.tokenizeRoot()
		case stateVariable:
			tok, cont, err = l.tokenizeBlockOrVar(sentinelVariable)
		case stateBlock:
			tok, cont, err = l.tokenizeBlockOrVar(sentinelBlock)
		case stateLineStatement:
			tok, cont, err = l.tokenizeBlockOrVar(sentinelLineStatement)
		}

		if err != nil {
			return nil, err
		}
		if cont {
			continue
		}
		if tok != nil {
			return tok, nil
		}
	}
}

func (l *Lexer) currentState() lexerState {
	if len(l.stack) == 0 {
		return stateTemplate
	}
	return l.stack[len(l.stack)-1]
}

func (l *Lexer) pushState(s lexerState) {
	l.stack = append(l.stack, s)
}

func (l *Lexer) popState() {
	if len(l.stack) > 0 {
		l.stack = l.stack[:len(l.stack)-1]
	}
}

type blockSentinel int

const (
	sentinelVariable blockSentinel = iota
	sentinelBlock
	sentinelLineStatement
)

// tokenizeRoot handles template data state.
func (l *Lexer) tokenizeRoot() (*Token, bool, error) {
	// Handle pending start marker
	if l.pendingStartMarker != nil {
		pm := l.pendingStartMarker
		l.pendingStartMarker = nil
		return l.handleStartMarkerWithPrefixStart(pm.marker, pm.length, pm.prefixStart)
	}

	// Handle trim leading whitespace from previous block
	if l.trimLeadingWhitespace {
		l.trimLeadingWhitespace = false
		l.skipWhitespace()
	}

	l.markStart()

	// Find next start marker
	match := l.findStartMarker()
	if match == nil {
		// No marker found, rest is template data
		if l.pos < len(l.source) {
			text := l.advance(len(l.source) - l.pos)
			tok := l.makeToken(TokenTemplateData, text)
			return &tok, false, nil
		}
		return nil, false, nil
	}

	// We found a marker at match.offset
	marker, offset, length, ws := match.marker, match.offset, match.length, match.ws

	l.pendingStartMarker = &pendingMarker{marker: marker, length: length, prefixStart: match.prefixStart}

	// Determine how much template data to emit before the marker
	var lead string
	var span Span
	switch ws {
	case wsDefault:
		if l.shouldLstripBlock(marker, l.source[:l.pos+offset]) {
			// Strip trailing whitespace from the line
			peeked := l.rest()[:offset]
			trimmed := lstripBlock(peeked)
			lead = l.advance(len(trimmed))
			span = l.span() // Span ends here, before the stripped whitespace
			l.advance(len(peeked) - len(trimmed)) // Skip the whitespace
		} else {
			lead = l.advance(offset)
			span = l.span()
		}
	case wsPreserve:
		lead = l.advance(offset)
		span = l.span()
	case wsRemove:
		// Trim trailing whitespace before the marker
		peeked := l.rest()[:offset]
		trimmed := strings.TrimRight(peeked, " \t\n\r")
		lead = l.advance(len(trimmed))
		span = l.span() // Span ends here, before the stripped whitespace
		l.advance(len(peeked) - len(trimmed))
	}

	if lead == "" {
		return nil, true, nil // continue to handle start marker
	}

	tok := Token{
		Type:  TokenTemplateData,
		Value: lead,
		Span:  span,
	}
	return &tok, false, nil
}

type markerMatch struct {
	offset      int
	marker      startMarker
	length      int
	ws          whitespaceMode
	prefixStart int // For line markers: offset within match where the actual prefix starts
}

func (l *Lexer) findStartMarker() *markerMatch {
	rest := l.rest()
	offset := 0

	for offset < len(rest) {
		// Find the earliest of: {{ {% {# line_statement_prefix line_comment_prefix
		braceIdx := strings.IndexByte(rest[offset:], '{')
		if braceIdx >= 0 {
			braceIdx += offset
		}

		// Check for line statement/comment prefix (must be at start of line)
		lineMarker := l.findLineMarker(rest, offset)

		// Determine which comes first
		if braceIdx < 0 && lineMarker == nil {
			return nil
		}

		if lineMarker != nil && (braceIdx < 0 || lineMarker.offset <= braceIdx) {
			return lineMarker
		}

		if braceIdx < 0 {
			return lineMarker
		}

		// Process brace marker
		idx := braceIdx
		if idx+1 >= len(rest) {
			return nil
		}

		nextChar := rest[idx+1]
		var marker startMarker
		var baseLen int

		switch nextChar {
		case '{':
			marker = markerVariable
			baseLen = 2
		case '%':
			marker = markerBlock
			baseLen = 2
		case '#':
			marker = markerComment
			baseLen = 2
		default:
			offset = idx + 1
			continue
		}

		// Check for whitespace control character
		var ws whitespaceMode
		if idx+baseLen < len(rest) {
			ws = whitespaceFromByte(rest[idx+baseLen])
		}

		length := baseLen
		if ws != wsDefault {
			length++
		}

		return &markerMatch{
			offset: idx,
			marker: marker,
			length: length,
			ws:     ws,
		}
	}

	return nil
}

func (l *Lexer) findLineMarker(rest string, offset int) *markerMatch {
	lineStatementPrefix := l.syntax.LineStatementPrefix
	lineCommentPrefix := l.syntax.LineCommentPrefix

	if lineStatementPrefix == "" && lineCommentPrefix == "" {
		return nil
	}

	// Track the best (earliest) match found
	var bestMatch *markerMatch

	// Search for line comment prefix ANYWHERE (not just at line start)
	if lineCommentPrefix != "" {
		idx := strings.Index(rest[offset:], lineCommentPrefix)
		if idx >= 0 {
			matchOffset := offset + idx
			bestMatch = &markerMatch{
				offset:      matchOffset,
				marker:      markerLineComment,
				length:      len(lineCommentPrefix),
				ws:          wsDefault,
				prefixStart: 0,
			}
		}
	}

	// Search for line statement prefix at LINE START only
	if lineStatementPrefix != "" {
		for i := offset; i < len(rest); i++ {
			// Check if we're at start of line
			atLineStart := false
			if i == 0 {
				atLineStart = l.pos == 0 || (l.pos > 0 && l.source[l.pos-1] == '\n')
			} else {
				atLineStart = rest[i-1] == '\n'
			}

			if !atLineStart {
				// Skip to next newline
				nlIdx := strings.IndexByte(rest[i:], '\n')
				if nlIdx < 0 {
					break
				}
				i += nlIdx
				continue
			}

			// At start of line - skip leading whitespace
			lineStart := i
			for i < len(rest) && (rest[i] == ' ' || rest[i] == '\t') {
				i++
			}
			prefixStart := i

			// Check for line statement prefix
			if strings.HasPrefix(rest[i:], lineStatementPrefix) {
				// Check if this is earlier than the line comment match
				if bestMatch == nil || lineStart < bestMatch.offset {
					bestMatch = &markerMatch{
						offset:      lineStart,
						marker:      markerLineStatement,
						length:      (i - lineStart) + len(lineStatementPrefix),
						ws:          wsDefault,
						prefixStart: prefixStart - lineStart,
					}
				}
				break // Found a line statement match, stop searching
			}

			// If there's a line comment prefix that's a prefix of line statement prefix (e.g., # vs ##),
			// and we found ## but not #, continue searching
		}
	}

	// If we found a line comment, check if a line statement comes earlier
	// Also need to handle the case where ## and # both exist and ## comes first
	// In that case, ## should win because it's longer
	if bestMatch != nil && bestMatch.marker == markerLineComment && lineStatementPrefix != "" {
		// Check if there's a line statement that comes BEFORE the line comment
		// but the line comment has a longer prefix (e.g., ## vs #)
		// In that case, we need to check the actual character sequence
		lcOffset := bestMatch.offset
		for i := offset; i <= lcOffset; i++ {
			// Check if we're at start of line
			atLineStart := false
			if i == 0 {
				atLineStart = l.pos == 0 || (l.pos > 0 && l.source[l.pos-1] == '\n')
			} else {
				atLineStart = rest[i-1] == '\n'
			}

			if !atLineStart {
				continue
			}

			// At start of line - skip leading whitespace
			lineStart := i
			j := i
			for j < len(rest) && (rest[j] == ' ' || rest[j] == '\t') {
				j++
			}
			prefixStart := j

			// Check for line statement prefix
			if j < len(rest) && strings.HasPrefix(rest[j:], lineStatementPrefix) {
				// Found a line statement - check if it's earlier or at same position as line comment
				if lineStart < lcOffset {
					bestMatch = &markerMatch{
						offset:      lineStart,
						marker:      markerLineStatement,
						length:      (j - lineStart) + len(lineStatementPrefix),
						ws:          wsDefault,
						prefixStart: prefixStart - lineStart,
					}
					break
				} else if lineStart == lcOffset && strings.HasPrefix(lineCommentPrefix, lineStatementPrefix) {
					// Same position but line comment prefix is longer (e.g., ## includes #)
					// Keep line comment (it's more specific)
				} else if lineStart == lcOffset {
					// Same position, line statement wins if it's not a prefix of line comment
					bestMatch = &markerMatch{
						offset:      lineStart,
						marker:      markerLineStatement,
						length:      (j - lineStart) + len(lineStatementPrefix),
						ws:          wsDefault,
						prefixStart: prefixStart - lineStart,
					}
					break
				}
			}
		}
	}

	return bestMatch
}

func (l *Lexer) handleStartMarker(marker startMarker, skip int) (*Token, bool, error) {
	return l.handleStartMarkerWithPrefixStart(marker, skip, 0)
}

func (l *Lexer) handleStartMarkerWithPrefixStart(marker startMarker, skip int, prefixStart int) (*Token, bool, error) {
	switch marker {
	case markerComment:
		// Find end of comment
		rest := l.rest()[skip:]
		endIdx := strings.Index(rest, l.syntax.CommentEnd)
		if endIdx < 0 {
			l.advance(len(l.rest()))
			return nil, false, l.syntaxError("unexpected end of comment")
		}

		// Check for whitespace control before comment end
		ws := wsDefault
		if endIdx > 0 {
			ws = whitespaceFromByte(rest[endIdx-1])
		}

		l.advance(skip + endIdx + len(l.syntax.CommentEnd))
		l.handleTailWhitespace(ws)
		return nil, true, nil // continue

	case markerVariable:
		l.markStart()
		l.advance(skip)
		l.pushState(stateVariable)
		tok := l.makeToken(TokenVariableStart, l.syntax.VarStart)
		return &tok, false, nil

	case markerBlock:
		// Check for raw block
		blockContent := l.rest()[skip:]
		if rawLen, wsStart := l.skipBasicTag(blockContent, "raw"); rawLen > 0 {
			l.advance(skip + rawLen)
			return l.handleRawTag(wsStart)
		}

		l.markStart()
		l.advance(skip)
		l.pushState(stateBlock)
		tok := l.makeToken(TokenBlockStart, l.syntax.BlockStart)
		return &tok, false, nil

	case markerLineStatement:
		// Skip whitespace before prefix
		l.advance(prefixStart)
		l.markStart()
		// Skip the prefix itself
		l.advance(skip - prefixStart)
		l.pushState(stateLineStatement)
		tok := l.makeToken(TokenBlockStart, l.syntax.LineStatementPrefix)
		return &tok, false, nil

	case markerLineComment:
		// Skip everything until end of line including the newline
		rest := l.rest()[skip:]
		nlIdx := strings.IndexByte(rest, '\n')
		if nlIdx < 0 {
			l.advance(len(l.rest()))
		} else {
			l.advance(skip + nlIdx + 1)
		}
		return nil, true, nil // continue
	}

	return nil, false, nil
}

func (l *Lexer) handleRawTag(wsStart whitespaceMode) (*Token, bool, error) {
	l.markStart()

	// Find {% endraw %}
	rest := l.rest()
	ptr := 0

	for {
		blockIdx := strings.Index(rest[ptr:], l.syntax.BlockStart)
		if blockIdx < 0 {
			l.advance(len(rest))
			return nil, false, l.syntaxError("unexpected end of raw block")
		}
		blockIdx += ptr // Convert to absolute position in rest
		
		// Position right after {%
		afterBlockStart := blockIdx + len(l.syntax.BlockStart)

		// Check if this is endraw
		remaining := rest[afterBlockStart:]
		if endrawLen, wsNext := l.skipBasicTag(remaining, "endraw"); endrawLen > 0 {
			// Check for whitespace control right after {% (before endraw)
			ws := wsDefault
			if afterBlockStart < len(rest) {
				ws = whitespaceFromByte(rest[afterBlockStart])
			}

			end := blockIdx
			result := rest[:end]

			// Apply wsStart trimming (after raw tag)
			switch wsStart {
			case wsDefault:
				if l.whitespace.TrimBlocks {
					result = strings.TrimPrefix(result, "\r")
					result = strings.TrimPrefix(result, "\n")
				}
			case wsRemove:
				result = strings.TrimLeft(result, " \t\n\r")
			}

			// Apply ws trimming (before endraw tag)
			switch ws {
			case wsDefault:
				if l.whitespace.LstripBlocks {
					result = lstripBlock(result)
				}
			case wsRemove:
				result = strings.TrimRight(result, " \t\n\r")
			}

			l.advance(end)
			span := l.span()
			l.advance(len(l.syntax.BlockStart) + endrawLen)
			l.handleTailWhitespace(wsNext)

			tok := Token{
				Type:  TokenTemplateData,
				Value: result,
				Span:  span,
			}
			return &tok, false, nil
		}
		
		ptr = afterBlockStart
	}
}

// skipBasicTag checks if the string starts with a simple tag like "raw" or "endraw"
// Returns the length to skip and the whitespace mode at end.
func (l *Lexer) skipBasicTag(s string, name string) (int, whitespaceMode) {
	ptr := s

	// Skip optional whitespace control at start
	if len(ptr) > 0 && (ptr[0] == '-' || ptr[0] == '+') {
		ptr = ptr[1:]
	}

	// Skip whitespace
	ptr = strings.TrimLeft(ptr, " \t\n\r")

	// Check for name
	if !strings.HasPrefix(ptr, name) {
		return 0, wsDefault
	}
	ptr = ptr[len(name):]

	// After name must be whitespace or end delimiter
	if len(ptr) > 0 && isIdentPart(ptr[0]) {
		return 0, wsDefault
	}

	// Skip whitespace
	ptr = strings.TrimLeft(ptr, " \t\n\r")

	// Check for whitespace control before end
	ws := wsDefault
	if len(ptr) > 0 && (ptr[0] == '-' || ptr[0] == '+') {
		ws = whitespaceFromByte(ptr[0])
		ptr = ptr[1:]
	}

	// Check for block end
	if !strings.HasPrefix(ptr, l.syntax.BlockEnd) {
		return 0, wsDefault
	}
	ptr = ptr[len(l.syntax.BlockEnd):]

	return len(s) - len(ptr), ws
}

func (l *Lexer) handleTailWhitespace(ws whitespaceMode) {
	switch ws {
	case wsPreserve:
		// Do nothing
	case wsDefault:
		l.skipNewlineIfTrimBlocks()
	case wsRemove:
		l.trimLeadingWhitespace = true
	}
}

func (l *Lexer) skipNewlineIfTrimBlocks() {
	if l.whitespace.TrimBlocks {
		rest := l.rest()
		if strings.HasPrefix(rest, "\r") {
			l.advance(1)
			rest = l.rest()
		}
		if strings.HasPrefix(rest, "\n") {
			l.advance(1)
		}
	}
}

func (l *Lexer) shouldLstripBlock(marker startMarker, prefix string) bool {
	// Line statements/comments always lstrip
	if marker == markerLineStatement || marker == markerLineComment {
		return true
	}

	if l.whitespace.LstripBlocks && marker != markerVariable {
		// Only strip if we're at the start of a line
		for i := len(prefix) - 1; i >= 0; i-- {
			c := prefix[i]
			if c == '\n' || c == '\r' {
				return true
			} else if c != ' ' && c != '\t' {
				return false
			}
		}
		// At start of file
		return true
	}
	return false
}

// tokenizeBlockOrVar handles tokens inside {% %} or {{ }}.
func (l *Lexer) tokenizeBlockOrVar(sentinel blockSentinel) (*Token, bool, error) {
	// For line statements, check for end of line first (before skipping whitespace)
	if sentinel == sentinelLineStatement && l.parenBalance == 0 {
		rest := l.rest()
		// Skip horizontal whitespace only
		skipLen := 0
		for skipLen < len(rest) && (rest[skipLen] == ' ' || rest[skipLen] == '\t') {
			skipLen++
		}

		// Check for newline or end of input
		if skipLen < len(rest) && (rest[skipLen] == '\n' || rest[skipLen] == '\r') {
			l.markStart()
			l.advance(skipLen)
			// Advance past the newline(s)
			if l.rest() != "" && l.rest()[0] == '\r' {
				l.advance(1)
			}
			if l.rest() != "" && l.rest()[0] == '\n' {
				l.advance(1)
			}
			tok := l.makeToken(TokenBlockEnd, "\n")
			l.popState()
			return &tok, false, nil
		} else if skipLen >= len(rest) {
			// End of input
			l.markStart()
			l.advance(skipLen)
			tok := l.makeToken(TokenBlockEnd, "")
			l.popState()
			return &tok, false, nil
		}
	}

	// Skip whitespace inside blocks
	l.skipWhitespaceChars()

	if l.atEnd() {
		return nil, false, nil
	}

	l.markStart()
	rest := l.rest()

	// Check for block/variable end with optional whitespace control
	if l.parenBalance == 0 {
		switch sentinel {
		case sentinelBlock:
			// Check for -/+ %} or %}
			if len(rest) > 0 && (rest[0] == '-' || rest[0] == '+') && strings.HasPrefix(rest[1:], l.syntax.BlockEnd) {
				wasMinus := rest[0] == '-'
				l.popState()
				l.advance(1 + len(l.syntax.BlockEnd))
				tok := l.makeToken(TokenBlockEnd, string(rest[0])+l.syntax.BlockEnd)
				if wasMinus {
					l.trimLeadingWhitespace = true
				}
				return &tok, false, nil
			}
			if strings.HasPrefix(rest, l.syntax.BlockEnd) {
				l.popState()
				l.advance(len(l.syntax.BlockEnd))
				tok := l.makeToken(TokenBlockEnd, l.syntax.BlockEnd)
				l.skipNewlineIfTrimBlocks()
				return &tok, false, nil
			}

		case sentinelVariable:
			// Check for -/+ }} or }}
			if len(rest) > 0 && (rest[0] == '-' || rest[0] == '+') && strings.HasPrefix(rest[1:], l.syntax.VarEnd) {
				wasMinus := rest[0] == '-'
				l.popState()
				l.advance(1 + len(l.syntax.VarEnd))
				if wasMinus {
					l.trimLeadingWhitespace = true
				}
				tok := l.makeToken(TokenVariableEnd, "-"+l.syntax.VarEnd)
				return &tok, false, nil
			}
			if strings.HasPrefix(rest, l.syntax.VarEnd) {
				l.popState()
				l.advance(len(l.syntax.VarEnd))
				tok := l.makeToken(TokenVariableEnd, l.syntax.VarEnd)
				return &tok, false, nil
			}
		}
	}

	// Two-character operators
	if len(rest) >= 2 {
		op2 := rest[:2]
		var tok *Token
		switch op2 {
		case "//":
			l.advance(2)
			t := l.makeToken(TokenFloorDiv, "//")
			tok = &t
		case "**":
			l.advance(2)
			t := l.makeToken(TokenPow, "**")
			tok = &t
		case "==":
			l.advance(2)
			t := l.makeToken(TokenEq, "==")
			tok = &t
		case "!=":
			l.advance(2)
			t := l.makeToken(TokenNe, "!=")
			tok = &t
		case ">=":
			l.advance(2)
			t := l.makeToken(TokenGe, ">=")
			tok = &t
		case "<=":
			l.advance(2)
			t := l.makeToken(TokenLe, "<=")
			tok = &t
		}
		if tok != nil {
			return tok, false, nil
		}
	}

	// Single character operators
	ch := rest[0]
	switch ch {
	case '+':
		l.advance(1)
		tok := l.makeToken(TokenPlus, "+")
		return &tok, false, nil
	case '-':
		l.advance(1)
		tok := l.makeToken(TokenMinus, "-")
		return &tok, false, nil
	case '*':
		l.advance(1)
		tok := l.makeToken(TokenMul, "*")
		return &tok, false, nil
	case '/':
		l.advance(1)
		tok := l.makeToken(TokenDiv, "/")
		return &tok, false, nil
	case '%':
		l.advance(1)
		tok := l.makeToken(TokenMod, "%")
		return &tok, false, nil
	case '~':
		l.advance(1)
		tok := l.makeToken(TokenTilde, "~")
		return &tok, false, nil
	case '<':
		l.advance(1)
		tok := l.makeToken(TokenLt, "<")
		return &tok, false, nil
	case '>':
		l.advance(1)
		tok := l.makeToken(TokenGt, ">")
		return &tok, false, nil
	case '=':
		l.advance(1)
		tok := l.makeToken(TokenAssign, "=")
		return &tok, false, nil
	case '.':
		l.advance(1)
		tok := l.makeToken(TokenDot, ".")
		return &tok, false, nil
	case ',':
		l.advance(1)
		tok := l.makeToken(TokenComma, ",")
		return &tok, false, nil
	case ':':
		l.advance(1)
		tok := l.makeToken(TokenColon, ":")
		return &tok, false, nil
	case '|':
		l.advance(1)
		tok := l.makeToken(TokenPipe, "|")
		return &tok, false, nil
	case '(':
		l.parenBalance++
		l.advance(1)
		tok := l.makeToken(TokenParenOpen, "(")
		return &tok, false, nil
	case ')':
		l.parenBalance--
		l.advance(1)
		tok := l.makeToken(TokenParenClose, ")")
		return &tok, false, nil
	case '[':
		l.parenBalance++
		l.advance(1)
		tok := l.makeToken(TokenBracketOpen, "[")
		return &tok, false, nil
	case ']':
		l.parenBalance--
		l.advance(1)
		tok := l.makeToken(TokenBracketClose, "]")
		return &tok, false, nil
	case '{':
		l.parenBalance++
		l.advance(1)
		tok := l.makeToken(TokenBraceOpen, "{")
		return &tok, false, nil
	case '}':
		l.parenBalance--
		l.advance(1)
		tok := l.makeToken(TokenBraceClose, "}")
		return &tok, false, nil
	case '"', '\'':
		return l.lexString(ch)
	}

	// Numbers
	if isDigit(ch) {
		return l.lexNumber()
	}

	// Identifiers
	if isIdentStart(ch) {
		return l.lexIdent()
	}

	return nil, false, l.syntaxError(fmt.Sprintf("unexpected character %q", ch))
}

// lexString lexes a string literal.
func (l *Lexer) lexString(quote byte) (*Token, bool, error) {
	l.advance(1) // skip opening quote

	var sb strings.Builder
	hasEscapes := false

	for !l.atEnd() {
		ch := l.rest()[0]
		if ch == quote {
			l.advance(1)
			tok := l.makeToken(TokenString, sb.String())
			return &tok, false, nil
		}
		if ch == '\\' {
			hasEscapes = true
			l.advance(1)
			if l.atEnd() {
				return nil, false, l.syntaxError("unexpected end of string")
			}
			escaped := l.rest()[0]
			l.advance(1)
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
			case '0':
				sb.WriteByte(0)
			case 'x':
				// Hex escape \xNN
				if len(l.rest()) < 2 {
					return nil, false, l.syntaxError("invalid hex escape")
				}
				hex := l.rest()[:2]
				val, err := strconv.ParseUint(hex, 16, 8)
				if err != nil {
					return nil, false, l.syntaxError("invalid hex escape")
				}
				sb.WriteByte(byte(val))
				l.advance(2)
			case 'u':
				// Unicode escape \uNNNN (4 hex digits)
				if len(l.rest()) < 4 {
					return nil, false, l.syntaxError("invalid unicode escape")
				}
				hex := l.rest()[:4]
				val, err := strconv.ParseUint(hex, 16, 32)
				if err != nil {
					return nil, false, l.syntaxError("invalid unicode escape")
				}
				sb.WriteRune(rune(val))
				l.advance(4)
			case 'U':
				// Unicode escape \UNNNNNNNN (8 hex digits)
				if len(l.rest()) < 8 {
					return nil, false, l.syntaxError("invalid unicode escape")
				}
				hex := l.rest()[:8]
				val, err := strconv.ParseUint(hex, 16, 32)
				if err != nil {
					return nil, false, l.syntaxError("invalid unicode escape")
				}
				sb.WriteRune(rune(val))
				l.advance(8)
			default:
				// Unknown escape, keep both characters
				sb.WriteByte('\\')
				sb.WriteByte(escaped)
			}
		} else {
			sb.WriteByte(ch)
			l.advance(1)
		}
	}

	_ = hasEscapes // used in Rust for optimization
	return nil, false, l.syntaxError("unexpected end of string")
}

// lexNumber lexes an integer or float literal, including hex, octal, binary, and underscores.
func (l *Lexer) lexNumber() (*Token, bool, error) {
	rest := l.rest()

	// Determine radix
	radix := 10
	prefixLen := 0

	if len(rest) >= 2 {
		switch rest[:2] {
		case "0b", "0B":
			radix = 2
			prefixLen = 2
		case "0o", "0O":
			radix = 8
			prefixLen = 2
		case "0x", "0X":
			radix = 16
			prefixLen = 2
		}
	}

	// State machine for parsing numbers
	type numState int
	const (
		stateRadixInt numState = iota // after 0x, 0b, 0o
		stateInt
		stateFraction // after .
		stateExponent // after e/E
		stateExpSign  // after e+/e-
	)

	var state numState
	if radix == 10 {
		state = stateInt
	} else {
		state = stateRadixInt
	}

	numLen := prefixLen
	hasUnderscore := false

	for i := prefixLen; i < len(rest); i++ {
		c := rest[i]
		switch state {
		case stateRadixInt:
			switch {
			case isDigitForRadix(c, radix):
				numLen++
			case c == '_':
				hasUnderscore = true
				numLen++
			default:
				goto done
			}

		case stateInt:
			switch {
			case isDigit(c):
				numLen++
			case c == '_':
				hasUnderscore = true
				numLen++
			case c == '.' && i+1 < len(rest) && isDigit(rest[i+1]):
				state = stateFraction
				numLen++
			case c == 'e' || c == 'E':
				state = stateExponent
				numLen++
			default:
				goto done
			}

		case stateFraction:
			switch {
			case isDigit(c):
				numLen++
			case c == '_':
				hasUnderscore = true
				numLen++
			case c == 'e' || c == 'E':
				state = stateExponent
				numLen++
			default:
				goto done
			}

		case stateExponent:
			switch {
			case c == '+' || c == '-':
				state = stateExpSign
				numLen++
			case isDigit(c):
				state = stateExpSign
				numLen++
			case c == '_':
				hasUnderscore = true
				state = stateExpSign
				numLen++
			default:
				goto done
			}

		case stateExpSign:
			switch {
			case isDigit(c):
				numLen++
			case c == '_':
				hasUnderscore = true
				numLen++
			default:
				goto done
			}
		}
	}

done:
	isFloat := state == stateFraction || state == stateExponent || state == stateExpSign

	numStr := rest[:numLen]
	l.advance(numLen)

	// Check for trailing underscore
	if hasUnderscore && strings.HasSuffix(numStr, "_") {
		return nil, false, l.syntaxError("'_' may not occur at end of number")
	}

	// Remove underscores for parsing
	cleanNum := numStr
	if hasUnderscore {
		cleanNum = strings.ReplaceAll(numStr, "_", "")
	}

	// Strip prefix for parsing non-decimal
	parseStr := cleanNum
	if prefixLen > 0 {
		parseStr = cleanNum[prefixLen:]
	}

	if isFloat {
		// Parse the float to get its actual value
		floatVal, err := strconv.ParseFloat(strings.ReplaceAll(cleanNum, "_", ""), 64)
		if err != nil {
			return nil, false, l.syntaxError("invalid float")
		}
		// Format float like Rust does (always with decimal point)
		floatStr := strconv.FormatFloat(floatVal, 'f', -1, 64)
		if !strings.Contains(floatStr, ".") {
			floatStr += ".0"
		}
		tok := l.makeToken(TokenFloat, floatStr)
		return &tok, false, nil
	}

	// Try parsing as integer
	value, parseErr := strconv.ParseUint(parseStr, radix, 64)

	if parseErr != nil {
		// Number too large for u64, emit as Int128
		// For display, we need to convert to decimal if it's hex/oct/bin
		if radix != 10 {
			// Use big.Int to convert
			bigVal := new(big.Int)
			_, ok := bigVal.SetString(parseStr, radix)
			if !ok {
				return nil, false, l.syntaxError("invalid integer (too large)")
			}
			tok := l.makeToken(TokenInt128, bigVal.String())
			return &tok, false, nil
		}
		tok := l.makeToken(TokenInt128, parseStr)
		return &tok, false, nil
	}

	tok := l.makeToken(TokenInteger, fmt.Sprintf("%d", value))
	return &tok, false, nil
}

func isDigitForRadix(c byte, radix int) bool {
	switch radix {
	case 2:
		return c == '0' || c == '1'
	case 8:
		return c >= '0' && c <= '7'
	case 10:
		return isDigit(c)
	case 16:
		return isDigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
	}
	return false
}

// lexIdent lexes an identifier.
func (l *Lexer) lexIdent() (*Token, bool, error) {
	start := 0
	rest := l.rest()

	for i := 0; i < len(rest); i++ {
		if !isIdentPart(rest[i]) {
			break
		}
		start = i + 1
	}

	value := rest[:start]
	l.advance(start)
	tok := l.makeToken(TokenIdent, value)
	return &tok, false, nil
}

// Helper methods

func (l *Lexer) atEnd() bool {
	return l.pos >= len(l.source)
}

func (l *Lexer) rest() string {
	if l.pos >= len(l.source) {
		return ""
	}
	return l.source[l.pos:]
}

func (l *Lexer) advance(n int) string {
	if n <= 0 {
		return ""
	}
	start := l.pos
	end := l.pos + n
	if end > len(l.source) {
		end = len(l.source)
	}

	skipped := l.source[start:end]
	for _, c := range skipped {
		if c == '\n' {
			l.line++
			l.col = 0
		} else {
			if l.col < 65535 {
				l.col++
			}
		}
	}
	l.pos = end
	return skipped
}

func (l *Lexer) markStart() {
	l.start = l.pos
	l.startLine = l.line
	l.startCol = l.col
}

func (l *Lexer) span() Span {
	return Span{
		StartLine:   l.startLine,
		StartCol:    l.startCol,
		StartOffset: uint32(l.start),
		EndLine:     l.line,
		EndCol:      l.col,
		EndOffset:   uint32(l.pos),
	}
}

func (l *Lexer) makeToken(typ TokenType, value string) Token {
	return Token{
		Type:  typ,
		Value: value,
		Span:  l.span(),
	}
}

func (l *Lexer) skipWhitespace() {
	for !l.atEnd() {
		c := l.rest()[0]
		if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
			l.advance(1)
		} else {
			break
		}
	}
}

func (l *Lexer) skipWhitespaceChars() {
	for !l.atEnd() {
		c := l.rest()[0]
		if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
			l.advance(1)
		} else {
			break
		}
	}
}

func (l *Lexer) syntaxError(msg string) error {
	return fmt.Errorf("syntax error at line %d, col %d: %s", l.line, l.col, msg)
}

func lstripBlock(s string) string {
	// Trim trailing whitespace (but not newlines) from the end
	trimmed := strings.TrimRightFunc(s, func(r rune) bool {
		return r == ' ' || r == '\t'
	})
	// Only strip if what remains ends with a newline or is empty
	if trimmed == "" || strings.HasSuffix(trimmed, "\n") {
		return trimmed
	}
	return s
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
