package parser

import (
	"fmt"
	"math/big"
	"strconv"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/lexer"
)

const maxRecursion = 150

var reservedNames = map[string]bool{
	"true": true, "True": true,
	"false": true, "False": true,
	"none": true, "None": true,
	"loop": true, "self": true,
}

// Error represents a parse error.
type Error struct {
	Kind    string
	Detail  string
	Name    string
	Line    uint16
}

func (e *Error) Error() string {
	return fmt.Sprintf("%s: %s (line %d)", e.Kind, e.Detail, e.Line)
}

// Result represents the result of parsing: either an AST or an error.
type Result struct {
	Template *Template
	Err      *Error
}

// Parser parses Jinja2 templates.
type Parser struct {
	tokens   []lexer.Token
	pos      int
	filename string
	inMacro  bool
	inLoop   bool
	blocks   map[string]bool
	depth    int
	lastSpan Span
}

// Parse parses a template string and returns the AST or an error.
func Parse(source, filename string, syntax lexer.SyntaxConfig, whitespace lexer.WhitespaceConfig) (*Template, error) {
	tokens, err := lexer.Tokenize(source, syntax, whitespace)
	if err != nil {
		return nil, &Error{
			Kind:   "SyntaxError",
			Detail: err.Error(),
			Name:   filename,
			Line:   1,
		}
	}

	p := &Parser{
		tokens:   tokens,
		filename: filename,
		blocks:   make(map[string]bool),
	}

	tmpl, parseErr := p.parse()
	if parseErr != nil {
		return nil, parseErr
	}
	return tmpl, nil
}

// ParseDefault parses a template string using default config and returns the AST or an error.
func ParseDefault(source, filename string) Result {
	syntaxCfg := lexer.DefaultSyntax()
	whitespaceCfg := lexer.DefaultWhitespace()

	tmpl, err := Parse(source, filename, syntaxCfg, whitespaceCfg)
	if err != nil {
		if e, ok := err.(*Error); ok {
			return Result{Err: e}
		}
		return Result{Err: &Error{Kind: "ParseError", Detail: err.Error(), Name: filename}}
	}
	return Result{Template: tmpl}
}

func (p *Parser) parse() (*Template, *Error) {
	// Root template always starts at 0:0
	span := Span{StartLine: 0, StartCol: 0, StartOffset: 0}
	children, err := p.subparse(func(tok lexer.Token) bool { return false })
	if err != nil {
		return nil, err
	}
	return &Template{
		Children: children,
		span:     p.expandSpan(span),
	}, nil
}

func (p *Parser) current() *lexer.Token {
	if p.pos >= len(p.tokens) {
		return nil
	}
	return &p.tokens[p.pos]
}

func (p *Parser) advance() *lexer.Token {
	if p.pos >= len(p.tokens) {
		return nil
	}
	tok := &p.tokens[p.pos]
	p.lastSpan = tok.Span
	p.pos++
	return tok
}

func (p *Parser) currentSpan() Span {
	if tok := p.current(); tok != nil {
		return tok.Span
	}
	return p.lastSpan
}

func (p *Parser) expandSpan(start Span) Span {
	return Span{
		StartLine:   start.StartLine,
		StartCol:    start.StartCol,
		StartOffset: start.StartOffset,
		EndLine:     p.lastSpan.EndLine,
		EndCol:      p.lastSpan.EndCol,
		EndOffset:   p.lastSpan.EndOffset,
	}
}

func (p *Parser) syntaxError(msg string) *Error {
	span := p.currentSpan()
	return &Error{
		Kind:   "SyntaxError",
		Detail: msg,
		Name:   p.filename,
		Line:   span.StartLine,
	}
}

func (p *Parser) unexpected(got string, expected string) *Error {
	return p.syntaxError(fmt.Sprintf("unexpected %s, expected %s", got, expected))
}

func (p *Parser) unexpectedEOF(expected string) *Error {
	return p.syntaxError(fmt.Sprintf("unexpected end of input, expected %s", expected))
}

func (p *Parser) expect(typ lexer.TokenType, expected string) (*lexer.Token, *Error) {
	tok := p.advance()
	if tok == nil {
		return nil, p.unexpectedEOF(expected)
	}
	if tok.Type != typ {
		return nil, p.unexpected(tokenDescription(tok), expected)
	}
	return tok, nil
}

func (p *Parser) expectIdent(expected string) (string, Span, *Error) {
	tok := p.advance()
	if tok == nil {
		return "", Span{}, p.unexpectedEOF(expected)
	}
	if tok.Type != lexer.TokenIdent {
		return "", Span{}, p.unexpected(tokenDescription(tok), expected)
	}
	return tok.Value, tok.Span, nil
}

func (p *Parser) expectKeyword(kw string, expected string) *Error {
	tok := p.advance()
	if tok == nil {
		return p.unexpectedEOF(expected)
	}
	if tok.Type != lexer.TokenIdent || tok.Value != kw {
		return p.unexpected(tokenDescription(tok), expected)
	}
	return nil
}

func (p *Parser) skip(typ lexer.TokenType) bool {
	if tok := p.current(); tok != nil && tok.Type == typ {
		p.advance()
		return true
	}
	return false
}

func (p *Parser) skipKeyword(kw string) bool {
	if tok := p.current(); tok != nil && tok.Type == lexer.TokenIdent && tok.Value == kw {
		p.advance()
		return true
	}
	return false
}

func (p *Parser) matches(typ lexer.TokenType) bool {
	tok := p.current()
	return tok != nil && tok.Type == typ
}

func (p *Parser) matchesKeyword(kw string) bool {
	tok := p.current()
	return tok != nil && tok.Type == lexer.TokenIdent && tok.Value == kw
}

func (p *Parser) matchesAny(types ...lexer.TokenType) bool {
	tok := p.current()
	if tok == nil {
		return false
	}
	for _, t := range types {
		if tok.Type == t {
			return true
		}
	}
	return false
}

func (p *Parser) matchesAnyKeyword(keywords ...string) bool {
	tok := p.current()
	if tok == nil || tok.Type != lexer.TokenIdent {
		return false
	}
	for _, kw := range keywords {
		if tok.Value == kw {
			return true
		}
	}
	return false
}

func tokenDescription(tok *lexer.Token) string {
	switch tok.Type {
	case lexer.TokenIdent:
		return fmt.Sprintf("identifier")
	case lexer.TokenString:
		return "string"
	case lexer.TokenInteger, lexer.TokenInt128:
		return "integer"
	case lexer.TokenFloat:
		return "float"
	case lexer.TokenBlockEnd:
		return "end of block"
	case lexer.TokenVariableEnd:
		return "end of variable block"
	default:
		return fmt.Sprintf("`%s`", tok.Value)
	}
}

// --- Expression Parsing ---

func (p *Parser) parseExpr() (Expr, *Error) {
	p.depth++
	if p.depth > maxRecursion {
		return nil, p.syntaxError("template exceeds maximum recursion limits")
	}
	defer func() { p.depth-- }()
	return p.parseIfExpr()
}

func (p *Parser) parseExprNoIf() (Expr, *Error) {
	return p.parseOr()
}

func (p *Parser) parseIfExpr() (Expr, *Error) {
	span := p.lastSpan
	expr, err := p.parseOr()
	if err != nil {
		return nil, err
	}

	for p.skipKeyword("if") {
		testExpr, err := p.parseOr()
		if err != nil {
			return nil, err
		}
		var falseExpr Expr
		if p.skipKeyword("else") {
			falseExpr, err = p.parseIfExpr()
			if err != nil {
				return nil, err
			}
		}
		expr = &IfExpr{
			TestExpr:  testExpr,
			TrueExpr:  expr,
			FalseExpr: falseExpr,
			span:      p.expandSpan(span),
		}
		span = p.lastSpan
	}
	return expr, nil
}

func (p *Parser) parseOr() (Expr, *Error) {
	span := p.currentSpan()
	left, err := p.parseAnd()
	if err != nil {
		return nil, err
	}
	for p.skipKeyword("or") {
		right, err := p.parseAnd()
		if err != nil {
			return nil, err
		}
		left = &BinOp{Op: BinOpScOr, Left: left, Right: right, span: p.expandSpan(span)}
	}
	return left, nil
}

func (p *Parser) parseAnd() (Expr, *Error) {
	span := p.currentSpan()
	left, err := p.parseNot()
	if err != nil {
		return nil, err
	}
	for p.skipKeyword("and") {
		right, err := p.parseNot()
		if err != nil {
			return nil, err
		}
		left = &BinOp{Op: BinOpScAnd, Left: left, Right: right, span: p.expandSpan(span)}
	}
	return left, nil
}

func (p *Parser) parseNot() (Expr, *Error) {
	span := p.currentSpan()
	if p.skipKeyword("not") {
		expr, err := p.parseNot()
		if err != nil {
			return nil, err
		}
		return &UnaryOp{Op: UnaryNot, Expr: expr, span: p.expandSpan(span)}, nil
	}
	return p.parseCompare()
}

func (p *Parser) parseCompare() (Expr, *Error) {
	span := p.lastSpan
	expr, err := p.parseMath1()
	if err != nil {
		return nil, err
	}

	for {
		var op BinOpKind
		negated := false

		tok := p.current()
		if tok == nil {
			break
		}

		switch tok.Type {
		case lexer.TokenEq:
			op = BinOpEq
		case lexer.TokenNe:
			op = BinOpNe
		case lexer.TokenLt:
			op = BinOpLt
		case lexer.TokenLe:
			op = BinOpLte
		case lexer.TokenGt:
			op = BinOpGt
		case lexer.TokenGe:
			op = BinOpGte
		case lexer.TokenIdent:
			if tok.Value == "in" {
				op = BinOpIn
			} else if tok.Value == "not" {
				p.advance()
				if err := p.expectKeyword("in", "in"); err != nil {
					return nil, err
				}
				op = BinOpIn
				negated = true
			} else {
				return expr, nil
			}
		default:
			return expr, nil
		}

		if !negated {
			p.advance()
		}

		right, err := p.parseMath1()
		if err != nil {
			return nil, err
		}
		expr = &BinOp{Op: op, Left: expr, Right: right, span: p.expandSpan(span)}
		if negated {
			expr = &UnaryOp{Op: UnaryNot, Expr: expr, span: p.expandSpan(span)}
		}
		span = p.lastSpan
	}
	return expr, nil
}

func (p *Parser) parseMath1() (Expr, *Error) {
	span := p.currentSpan()
	left, err := p.parseConcat()
	if err != nil {
		return nil, err
	}
	for {
		var op BinOpKind
		switch {
		case p.skip(lexer.TokenPlus):
			op = BinOpAdd
		case p.skip(lexer.TokenMinus):
			op = BinOpSub
		default:
			return left, nil
		}
		right, err := p.parseConcat()
		if err != nil {
			return nil, err
		}
		left = &BinOp{Op: op, Left: left, Right: right, span: p.expandSpan(span)}
	}
}

func (p *Parser) parseConcat() (Expr, *Error) {
	span := p.currentSpan()
	left, err := p.parseMath2()
	if err != nil {
		return nil, err
	}
	for p.skip(lexer.TokenTilde) {
		right, err := p.parseMath2()
		if err != nil {
			return nil, err
		}
		left = &BinOp{Op: BinOpConcat, Left: left, Right: right, span: p.expandSpan(span)}
	}
	return left, nil
}

func (p *Parser) parseMath2() (Expr, *Error) {
	span := p.currentSpan()
	left, err := p.parsePow()
	if err != nil {
		return nil, err
	}
	for {
		var op BinOpKind
		switch {
		case p.skip(lexer.TokenMul):
			op = BinOpMul
		case p.skip(lexer.TokenDiv):
			op = BinOpDiv
		case p.skip(lexer.TokenFloorDiv):
			op = BinOpFloorDiv
		case p.skip(lexer.TokenMod):
			op = BinOpRem
		default:
			return left, nil
		}
		right, err := p.parsePow()
		if err != nil {
			return nil, err
		}
		left = &BinOp{Op: op, Left: left, Right: right, span: p.expandSpan(span)}
	}
}

func (p *Parser) parsePow() (Expr, *Error) {
	span := p.currentSpan()
	left, err := p.parseUnary()
	if err != nil {
		return nil, err
	}
	for p.skip(lexer.TokenPow) {
		right, err := p.parseUnary()
		if err != nil {
			return nil, err
		}
		left = &BinOp{Op: BinOpPow, Left: left, Right: right, span: p.expandSpan(span)}
	}
	return left, nil
}

func (p *Parser) parseUnary() (Expr, *Error) {
	span := p.currentSpan()
	expr, err := p.parseUnaryOnly()
	if err != nil {
		return nil, err
	}
	expr, err = p.parsePostfix(expr, span)
	if err != nil {
		return nil, err
	}
	return p.parseFilterExpr(expr)
}

func (p *Parser) parseUnaryOnly() (Expr, *Error) {
	span := p.currentSpan()
	if p.skip(lexer.TokenMinus) {
		expr, err := p.parseUnaryOnly()
		if err != nil {
			return nil, err
		}
		return &UnaryOp{Op: UnaryNeg, Expr: expr, span: p.expandSpan(span)}, nil
	}
	return p.parsePrimary()
}

func (p *Parser) parsePostfix(expr Expr, span Span) (Expr, *Error) {
	for {
		nextSpan := p.currentSpan()
		switch {
		case p.skip(lexer.TokenDot):
			name, _, err := p.expectIdent("identifier")
			if err != nil {
				return nil, err
			}
			expr = &GetAttr{Expr: expr, Name: name, span: p.expandSpan(span)}

		case p.skip(lexer.TokenBracketOpen):
			var start, stop, step Expr
			var isSlice bool
			var err *Error

			if !p.matches(lexer.TokenColon) {
				start, err = p.parseExpr()
				if err != nil {
					return nil, err
				}
			}
			if p.skip(lexer.TokenColon) {
				isSlice = true
				if !p.matchesAny(lexer.TokenBracketClose, lexer.TokenColon) {
					stop, err = p.parseExpr()
					if err != nil {
						return nil, err
					}
				}
				if p.skip(lexer.TokenColon) && !p.matches(lexer.TokenBracketClose) {
					step, err = p.parseExpr()
					if err != nil {
						return nil, err
					}
				}
			}
			if _, err := p.expect(lexer.TokenBracketClose, "`]`"); err != nil {
				return nil, err
			}

			if !isSlice {
				if start == nil {
					return nil, p.syntaxError("empty subscript")
				}
				expr = &GetItem{Expr: expr, SubscriptExpr: start, span: p.expandSpan(span)}
			} else {
				expr = &Slice{Expr: expr, Start: start, Stop: stop, Step: step, span: p.expandSpan(span)}
			}

		case p.matches(lexer.TokenParenOpen):
			args, err := p.parseArgs()
			if err != nil {
				return nil, err
			}
			expr = &Call{Expr: expr, Args: args, span: p.expandSpan(span)}

		default:
			return expr, nil
		}
		span = nextSpan
	}
}

func (p *Parser) parseFilterExpr(expr Expr) (Expr, *Error) {
	for {
		switch {
		case p.skip(lexer.TokenPipe):
			name, span, err := p.expectIdent("identifier")
			if err != nil {
				return nil, err
			}
			var args []CallArg
			if p.matches(lexer.TokenParenOpen) {
				args, err = p.parseArgs()
				if err != nil {
					return nil, err
				}
			}
			expr = &Filter{Name: name, Expr: expr, Args: args, span: p.expandSpan(span)}

		case p.matchesKeyword("is"):
			p.advance()
			negated := p.skipKeyword("not")
			name, span, err := p.expectIdent("identifier")
			if err != nil {
				return nil, err
			}
			var args []CallArg
			if p.matches(lexer.TokenParenOpen) {
				args, err = p.parseArgs()
				if err != nil {
					return nil, err
				}
			} else if p.matchesAny(lexer.TokenIdent, lexer.TokenString, lexer.TokenInteger,
				lexer.TokenInt128, lexer.TokenFloat, lexer.TokenPlus, lexer.TokenMinus,
				lexer.TokenBracketOpen, lexer.TokenBraceOpen) &&
				!p.matchesAnyKeyword("and", "or", "else", "is") {
				argSpan := p.currentSpan()
				argExpr, err := p.parseUnaryOnly()
				if err != nil {
					return nil, err
				}
				argExpr, err = p.parsePostfix(argExpr, argSpan)
				if err != nil {
					return nil, err
				}
				args = []CallArg{{Kind: CallArgPos, Value: argExpr}}
			}
			expr = &Test{Name: name, Expr: expr, Args: args, span: p.expandSpan(span)}
			if negated {
				expr = &UnaryOp{Op: UnaryNot, Expr: expr, span: p.expandSpan(span)}
			}

		default:
			return expr, nil
		}
	}
}

func (p *Parser) parseArgs() ([]CallArg, *Error) {
	var args []CallArg
	hasKwargs := false

	if _, err := p.expect(lexer.TokenParenOpen, "`(`"); err != nil {
		return nil, err
	}

	for {
		if p.skip(lexer.TokenParenClose) {
			break
		}
		if len(args) > 0 || hasKwargs {
			if _, err := p.expect(lexer.TokenComma, "`,`"); err != nil {
				return nil, err
			}
			if p.skip(lexer.TokenParenClose) {
				break
			}
		}

		// Check for splats
		var argType int // 0=regular, 1=splat, 2=kwargs_splat
		if p.skip(lexer.TokenPow) {
			argType = 2
		} else if p.skip(lexer.TokenMul) {
			argType = 1
		}

		expr, err := p.parseExpr()
		if err != nil {
			return nil, err
		}

		switch argType {
		case 0:
			// Check for keyword argument
			if v, ok := expr.(*Var); ok && p.skip(lexer.TokenAssign) {
				hasKwargs = true
				value, err := p.parseExprNoIf()
				if err != nil {
					return nil, err
				}
				args = append(args, CallArg{Kind: CallArgKwarg, Name: v.ID, Value: value})
			} else if hasKwargs {
				return nil, p.syntaxError("non-keyword arg after keyword arg")
			} else {
				args = append(args, CallArg{Kind: CallArgPos, Value: expr})
			}
		case 1:
			args = append(args, CallArg{Kind: CallArgPosSplat, Value: expr})
		case 2:
			args = append(args, CallArg{Kind: CallArgKwargSplat, Value: expr})
			hasKwargs = true
		}

		if len(args) > 2000 {
			return nil, p.syntaxError("Too many arguments in function call")
		}
	}

	return args, nil
}

func (p *Parser) parsePrimary() (Expr, *Error) {
	p.depth++
	if p.depth > maxRecursion {
		return nil, p.syntaxError("template exceeds maximum recursion limits")
	}
	defer func() { p.depth-- }()

	tok := p.advance()
	if tok == nil {
		return nil, p.unexpectedEOF("expression")
	}
	span := tok.Span

	switch tok.Type {
	case lexer.TokenIdent:
		switch tok.Value {
		case "true", "True":
			return &Const{Value: true, span: span}, nil
		case "false", "False":
			return &Const{Value: false, span: span}, nil
		case "none", "None":
			return &Const{Value: nil, span: span}, nil
		default:
			return &Var{ID: tok.Value, span: span}, nil
		}

	case lexer.TokenString:
		// Check for string concatenation
		val := tok.Value
		for p.matches(lexer.TokenString) {
			next := p.advance()
			val += next.Value
		}
		return &Const{Value: val, span: p.expandSpan(span)}, nil

	case lexer.TokenInteger:
		// Parse as int64 first
		val, err := strconv.ParseInt(tok.Value, 0, 64)
		if err == nil {
			return &Const{Value: val, span: span}, nil
		}
		// Overflow - parse as big.Int
		bi := new(big.Int)
		bi.SetString(tok.Value, 0)
		return &Const{Value: &BigInt{bi}, span: span}, nil

	case lexer.TokenInt128:
		// Parse as big.Int
		bi := new(big.Int)
		bi.SetString(tok.Value, 0)
		return &Const{Value: &BigInt{bi}, span: span}, nil

	case lexer.TokenFloat:
		val, _ := strconv.ParseFloat(tok.Value, 64)
		return &Const{Value: val, span: span}, nil

	case lexer.TokenParenOpen:
		return p.parseTupleOrExpr(span)

	case lexer.TokenBracketOpen:
		return p.parseListExpr(span)

	case lexer.TokenBraceOpen:
		return p.parseMapExpr(span)

	default:
		// Match Rust's format: just "unexpected X" without "expected Y"
		return nil, p.syntaxError(fmt.Sprintf("unexpected %s", tokenDescription(tok)))
	}
}

func (p *Parser) parseTupleOrExpr(span Span) (Expr, *Error) {
	// Empty tuple = empty list
	if p.skip(lexer.TokenParenClose) {
		return &List{Items: nil, span: p.expandSpan(span)}, nil
	}

	expr, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	if p.matches(lexer.TokenComma) {
		// It's a tuple (which we represent as a list)
		items := []Expr{expr}
		for {
			if p.skip(lexer.TokenParenClose) {
				break
			}
			if _, err := p.expect(lexer.TokenComma, "`,`"); err != nil {
				return nil, err
			}
			if p.skip(lexer.TokenParenClose) {
				break
			}
			item, err := p.parseExpr()
			if err != nil {
				return nil, err
			}
			items = append(items, item)
		}
		return &List{Items: items, span: p.expandSpan(span)}, nil
	}

	if _, err := p.expect(lexer.TokenParenClose, "`)`"); err != nil {
		return nil, err
	}
	return expr, nil
}

func (p *Parser) parseListExpr(span Span) (Expr, *Error) {
	var items []Expr
	for {
		if p.skip(lexer.TokenBracketClose) {
			break
		}
		if len(items) > 0 {
			if _, err := p.expect(lexer.TokenComma, "`,`"); err != nil {
				return nil, err
			}
			if p.skip(lexer.TokenBracketClose) {
				break
			}
		}
		item, err := p.parseExpr()
		if err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	return &List{Items: items, span: p.expandSpan(span)}, nil
}

func (p *Parser) parseMapExpr(span Span) (Expr, *Error) {
	var keys, values []Expr
	for {
		if p.skip(lexer.TokenBraceClose) {
			break
		}
		if len(keys) > 0 {
			if _, err := p.expect(lexer.TokenComma, "`,`"); err != nil {
				return nil, err
			}
			if p.skip(lexer.TokenBraceClose) {
				break
			}
		}
		key, err := p.parseExpr()
		if err != nil {
			return nil, err
		}
		if _, err := p.expect(lexer.TokenColon, "`:`"); err != nil {
			return nil, err
		}
		value, err := p.parseExpr()
		if err != nil {
			return nil, err
		}
		keys = append(keys, key)
		values = append(values, value)
	}
	return &Map{Keys: keys, Values: values, span: p.expandSpan(span)}, nil
}

// --- Statement Parsing ---

func (p *Parser) parseStmt() (Stmt, *Error) {
	p.depth++
	if p.depth > maxRecursion {
		return nil, p.syntaxError("template exceeds maximum recursion limits")
	}
	defer func() { p.depth-- }()

	tok := p.advance()
	if tok == nil {
		return nil, p.unexpectedEOF("block keyword")
	}
	span := tok.Span

	if tok.Type != lexer.TokenIdent {
		return nil, p.unexpected(tokenDescription(tok), "statement")
	}

	switch tok.Value {
	case "for":
		stmt, err := p.parseForStmt()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "if":
		stmt, err := p.parseIfCond()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "with":
		stmt, err := p.parseWithBlock()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "set":
		return p.parseSet(span)

	case "autoescape":
		stmt, err := p.parseAutoEscape()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "filter":
		stmt, err := p.parseFilterBlock()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "block":
		stmt, err := p.parseBlock()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "extends":
		stmt, err := p.parseExtends()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "include":
		stmt, err := p.parseInclude()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "import":
		stmt, err := p.parseImport()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "from":
		stmt, err := p.parseFromImport()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "macro":
		stmt, err := p.parseMacro()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "call":
		stmt, err := p.parseCallBlock(span)
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	case "continue":
		if !p.inLoop {
			return nil, p.syntaxError("'continue' must be placed inside a loop")
		}
		return &Continue{span: p.expandSpan(span)}, nil

	case "break":
		if !p.inLoop {
			return nil, p.syntaxError("'break' must be placed inside a loop")
		}
		return &Break{span: p.expandSpan(span)}, nil

	case "do":
		stmt, err := p.parseDo()
		if err != nil {
			return nil, err
		}
		stmt.span = p.expandSpan(span)
		return stmt, nil

	default:
		return nil, p.syntaxError(fmt.Sprintf("unknown statement %s", tok.Value))
	}
}

func (p *Parser) parseAssignName(dotted bool) (Expr, *Error) {
	name, span, err := p.expectIdent("identifier")
	if err != nil {
		return nil, err
	}
	if reservedNames[name] {
		return nil, p.syntaxError(fmt.Sprintf("cannot assign to reserved variable name %s", name))
	}
	var result Expr = &Var{ID: name, span: span}

	if dotted {
		for p.skip(lexer.TokenDot) {
			attr, attrSpan, err := p.expectIdent("identifier")
			if err != nil {
				return nil, err
			}
			result = &GetAttr{Expr: result, Name: attr, span: attrSpan}
		}
	}
	return result, nil
}

func (p *Parser) parseAssignment(dotted bool) (Expr, *Error) {
	span := p.currentSpan()
	var items []Expr
	isTuple := false

	for {
		if len(items) > 0 {
			if _, err := p.expect(lexer.TokenComma, "`,`"); err != nil {
				return nil, err
			}
		}
		if p.matchesAny(lexer.TokenParenClose, lexer.TokenVariableEnd, lexer.TokenBlockEnd) ||
			p.matchesKeyword("in") {
			break
		}

		var item Expr
		var err *Error
		if p.skip(lexer.TokenParenOpen) {
			item, err = p.parseAssignment(dotted)
			if err != nil {
				return nil, err
			}
			if _, err := p.expect(lexer.TokenParenClose, "`)`"); err != nil {
				return nil, err
			}
		} else {
			item, err = p.parseAssignName(dotted)
			if err != nil {
				return nil, err
			}
		}
		items = append(items, item)

		if p.matches(lexer.TokenComma) {
			isTuple = true
		} else {
			break
		}
	}

	if !isTuple && len(items) == 1 {
		return items[0], nil
	}
	return &List{Items: items, span: p.expandSpan(span)}, nil
}

func (p *Parser) parseForStmt() (*ForLoop, *Error) {
	oldInLoop := p.inLoop
	p.inLoop = true
	defer func() { p.inLoop = oldInLoop }()

	target, err := p.parseAssignment(false)
	if err != nil {
		return nil, err
	}

	if err := p.expectKeyword("in", "in"); err != nil {
		return nil, err
	}

	iter, err := p.parseExprNoIf()
	if err != nil {
		return nil, err
	}

	var filterExpr Expr
	if p.skipKeyword("if") {
		filterExpr, err = p.parseExpr()
		if err != nil {
			return nil, err
		}
	}

	recursive := p.skipKeyword("recursive")

	if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
		return nil, err
	}

	body, err := p.subparse(func(tok lexer.Token) bool {
		return tok.Type == lexer.TokenIdent && (tok.Value == "endfor" || tok.Value == "else")
	})
	if err != nil {
		return nil, err
	}

	var elseBody []Stmt
	if p.skipKeyword("else") {
		if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
			return nil, err
		}
		elseBody, err = p.subparse(func(tok lexer.Token) bool {
			return tok.Type == lexer.TokenIdent && tok.Value == "endfor"
		})
		if err != nil {
			return nil, err
		}
	}
	p.advance() // consume endfor

	return &ForLoop{
		Target:     target,
		Iter:       iter,
		FilterExpr: filterExpr,
		Recursive:  recursive,
		Body:       body,
		ElseBody:   elseBody,
	}, nil
}

func (p *Parser) parseIfCond() (*IfCond, *Error) {
	expr, err := p.parseExprNoIf()
	if err != nil {
		return nil, err
	}

	if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
		return nil, err
	}

	trueBody, err := p.subparse(func(tok lexer.Token) bool {
		return tok.Type == lexer.TokenIdent && (tok.Value == "endif" || tok.Value == "else" || tok.Value == "elif")
	})
	if err != nil {
		return nil, err
	}

	var falseBody []Stmt
	tok := p.advance()
	if tok != nil && tok.Type == lexer.TokenIdent {
		switch tok.Value {
		case "else":
			if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
				return nil, err
			}
			falseBody, err = p.subparse(func(tok lexer.Token) bool {
				return tok.Type == lexer.TokenIdent && tok.Value == "endif"
			})
			if err != nil {
				return nil, err
			}
			p.advance() // consume endif

		case "elif":
			span := tok.Span
			nested, err := p.parseIfCond()
			if err != nil {
				return nil, err
			}
			nested.span = p.expandSpan(span)
			falseBody = []Stmt{nested}
		}
	}

	return &IfCond{
		Expr:      expr,
		TrueBody:  trueBody,
		FalseBody: falseBody,
	}, nil
}

func (p *Parser) parseWithBlock() (*WithBlock, *Error) {
	var assignments []Assignment

	for !p.matches(lexer.TokenBlockEnd) {
		if len(assignments) > 0 {
			if _, err := p.expect(lexer.TokenComma, "comma"); err != nil {
				return nil, err
			}
		}

		var target Expr
		var err *Error
		if p.skip(lexer.TokenParenOpen) {
			target, err = p.parseAssignment(false)
			if err != nil {
				return nil, err
			}
			if _, err := p.expect(lexer.TokenParenClose, "`)`"); err != nil {
				return nil, err
			}
		} else {
			target, err = p.parseAssignName(false)
			if err != nil {
				return nil, err
			}
		}

		if _, err := p.expect(lexer.TokenAssign, "assignment operator"); err != nil {
			return nil, err
		}

		value, err := p.parseExpr()
		if err != nil {
			return nil, err
		}

		assignments = append(assignments, Assignment{Target: target, Value: value})
	}

	if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
		return nil, err
	}

	body, err := p.subparse(func(tok lexer.Token) bool {
		return tok.Type == lexer.TokenIdent && tok.Value == "endwith"
	})
	if err != nil {
		return nil, err
	}
	p.advance() // consume endwith

	return &WithBlock{Assignments: assignments, Body: body}, nil
}

func (p *Parser) parseSet(span Span) (Stmt, *Error) {
	target, err := p.parseAssignment(true)
	if err != nil {
		return nil, err
	}

	// Check for set block ({% set x %}...{% endset %})
	if p.matchesAny(lexer.TokenBlockEnd, lexer.TokenPipe) {
		var filter Expr
		if p.skip(lexer.TokenPipe) {
			filter, err = p.parseFilterChain()
			if err != nil {
				return nil, err
			}
		}
		if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
			return nil, err
		}
		body, err := p.subparse(func(tok lexer.Token) bool {
			return tok.Type == lexer.TokenIdent && tok.Value == "endset"
		})
		if err != nil {
			return nil, err
		}
		p.advance() // consume endset
		return &SetBlock{
			Target: target,
			Filter: filter,
			Body:   body,
			span:   p.expandSpan(span),
		}, nil
	}

	// Regular set statement
	if _, err := p.expect(lexer.TokenAssign, "assignment operator"); err != nil {
		return nil, err
	}

	expr, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	// Check for tuple assignment
	if p.skip(lexer.TokenComma) {
		tupleSpan := p.currentSpan()
		items := []Expr{expr}
		for {
			if p.matches(lexer.TokenBlockEnd) {
				break
			}
			item, err := p.parseExpr()
			if err != nil {
				return nil, err
			}
			items = append(items, item)
			if !p.skip(lexer.TokenComma) {
				break
			}
		}
		expr = &List{Items: items, span: p.expandSpan(tupleSpan)}
	}

	return &Set{Target: target, Expr: expr, span: p.expandSpan(span)}, nil
}

func (p *Parser) parseFilterChain() (Expr, *Error) {
	var filter Expr

	for !p.matches(lexer.TokenBlockEnd) {
		if filter != nil {
			if _, err := p.expect(lexer.TokenPipe, "`|`"); err != nil {
				return nil, err
			}
		}
		name, span, err := p.expectIdent("identifier")
		if err != nil {
			return nil, err
		}
		var args []CallArg
		if p.matches(lexer.TokenParenOpen) {
			args, err = p.parseArgs()
			if err != nil {
				return nil, err
			}
		}
		filter = &Filter{Name: name, Expr: filter, Args: args, span: p.expandSpan(span)}
	}

	if filter == nil {
		return nil, p.syntaxError("expected a filter")
	}
	return filter, nil
}

func (p *Parser) parseAutoEscape() (*AutoEscape, *Error) {
	enabled, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
		return nil, err
	}

	body, err := p.subparse(func(tok lexer.Token) bool {
		return tok.Type == lexer.TokenIdent && tok.Value == "endautoescape"
	})
	if err != nil {
		return nil, err
	}
	p.advance() // consume endautoescape

	return &AutoEscape{Enabled: enabled, Body: body}, nil
}

func (p *Parser) parseFilterBlock() (*FilterBlock, *Error) {
	filter, err := p.parseFilterChain()
	if err != nil {
		return nil, err
	}

	if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
		return nil, err
	}

	body, err := p.subparse(func(tok lexer.Token) bool {
		return tok.Type == lexer.TokenIdent && tok.Value == "endfilter"
	})
	if err != nil {
		return nil, err
	}
	p.advance() // consume endfilter

	return &FilterBlock{Filter: filter, Body: body}, nil
}

func (p *Parser) parseBlock() (*Block, *Error) {
	if p.inMacro {
		return nil, p.syntaxError("block tags in macros are not allowed")
	}
	oldInLoop := p.inLoop
	p.inLoop = false
	defer func() { p.inLoop = oldInLoop }()

	name, _, err := p.expectIdent("identifier")
	if err != nil {
		return nil, err
	}

	if p.blocks[name] {
		return nil, p.syntaxError(fmt.Sprintf("block '%s' defined twice", name))
	}
	p.blocks[name] = true

	if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
		return nil, err
	}

	body, err := p.subparse(func(tok lexer.Token) bool {
		return tok.Type == lexer.TokenIdent && tok.Value == "endblock"
	})
	if err != nil {
		return nil, err
	}
	p.advance() // consume endblock

	// Check for optional trailing block name
	if tok := p.current(); tok != nil && tok.Type == lexer.TokenIdent {
		if tok.Value != name {
			return nil, p.syntaxError(fmt.Sprintf("mismatching name on block. Got `%s`, expected `%s`", tok.Value, name))
		}
		p.advance()
	}

	return &Block{Name: name, Body: body}, nil
}

func (p *Parser) parseExtends() (*Extends, *Error) {
	name, err := p.parseExpr()
	if err != nil {
		return nil, err
	}
	return &Extends{Name: name}, nil
}

func (p *Parser) parseInclude() (*Include, *Error) {
	name, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	skippedContext := p.skipContextMarker()

	ignoreMissing := false
	if p.skipKeyword("ignore") {
		if err := p.expectKeyword("missing", "missing keyword"); err != nil {
			return nil, err
		}
		if !skippedContext {
			p.skipContextMarker()
		}
		ignoreMissing = true
	}

	return &Include{Name: name, IgnoreMissing: ignoreMissing}, nil
}

func (p *Parser) parseImport() (*Import, *Error) {
	expr, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	if err := p.expectKeyword("as", "as"); err != nil {
		return nil, err
	}

	name, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	p.skipContextMarker()

	return &Import{Expr: expr, Name: name}, nil
}

func (p *Parser) parseFromImport() (*FromImport, *Error) {
	expr, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	if err := p.expectKeyword("import", "import"); err != nil {
		return nil, err
	}

	var names []ImportName
	for {
		if p.skipContextMarker() || p.matches(lexer.TokenBlockEnd) {
			break
		}
		if len(names) > 0 {
			if _, err := p.expect(lexer.TokenComma, "`,`"); err != nil {
				return nil, err
			}
		}
		if p.skipContextMarker() || p.matches(lexer.TokenBlockEnd) {
			break
		}

		name, err := p.parseAssignName(false)
		if err != nil {
			return nil, err
		}

		var alias Expr
		if p.skipKeyword("as") {
			alias, err = p.parseAssignName(false)
			if err != nil {
				return nil, err
			}
		}

		names = append(names, ImportName{Name: name, Alias: alias})
	}

	return &FromImport{Expr: expr, Names: names}, nil
}

func (p *Parser) skipContextMarker() bool {
	if p.skipKeyword("with") || p.skipKeyword("without") {
		p.expectKeyword("context", "context")
		return true
	}
	return false
}

func (p *Parser) parseMacro() (*Macro, *Error) {
	name, _, err := p.expectIdent("identifier")
	if err != nil {
		return nil, err
	}

	if _, err := p.expect(lexer.TokenParenOpen, "`(`"); err != nil {
		return nil, err
	}

	var args, defaults []Expr
	if err := p.parseMacroArgsAndDefaults(&args, &defaults); err != nil {
		return nil, err
	}

	return p.parseMacroOrCallBlockBody(args, defaults, name)
}

func (p *Parser) parseMacroArgsAndDefaults(args, defaults *[]Expr) *Error {
	for {
		if p.skip(lexer.TokenParenClose) {
			break
		}
		if len(*args) > 0 {
			if _, err := p.expect(lexer.TokenComma, "`,`"); err != nil {
				return err
			}
			if p.skip(lexer.TokenParenClose) {
				break
			}
		}

		arg, err := p.parseAssignName(false)
		if err != nil {
			return err
		}
		*args = append(*args, arg)

		if p.skip(lexer.TokenAssign) {
			def, err := p.parseExpr()
			if err != nil {
				return err
			}
			*defaults = append(*defaults, def)
		} else if len(*defaults) > 0 {
			if _, err := p.expect(lexer.TokenAssign, "`=`"); err != nil {
				return err
			}
		}
	}
	return nil
}

func (p *Parser) parseMacroOrCallBlockBody(args, defaults []Expr, name string) (*Macro, *Error) {
	if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
		return nil, err
	}

	oldInLoop := p.inLoop
	oldInMacro := p.inMacro
	p.inLoop = false
	p.inMacro = true
	defer func() {
		p.inLoop = oldInLoop
		p.inMacro = oldInMacro
	}()

	endKeyword := "endmacro"
	if name == "" {
		endKeyword = "endcall"
		name = "caller"
	}

	body, err := p.subparse(func(tok lexer.Token) bool {
		return tok.Type == lexer.TokenIdent && tok.Value == endKeyword
	})
	if err != nil {
		return nil, err
	}
	p.advance() // consume end keyword

	return &Macro{Name: name, Args: args, Defaults: defaults, Body: body}, nil
}

func (p *Parser) parseCallBlock(span Span) (*CallBlock, *Error) {
	var args, defaults []Expr
	if p.skip(lexer.TokenParenOpen) {
		if err := p.parseMacroArgsAndDefaults(&args, &defaults); err != nil {
			return nil, err
		}
	}

	expr, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	call, ok := expr.(*Call)
	if !ok {
		return nil, p.syntaxError(fmt.Sprintf("expected call expression in call block, got %s", exprDescription(expr)))
	}
	callSpan := call.span

	macroDecl, err := p.parseMacroOrCallBlockBody(args, defaults, "")
	if err != nil {
		return nil, err
	}

	return &CallBlock{
		Call:      call,
		CallSpan:  callSpan,
		MacroDecl: macroDecl,
		MacroSpan: p.expandSpan(span),
	}, nil
}

func (p *Parser) parseDo() (*Do, *Error) {
	expr, err := p.parseExpr()
	if err != nil {
		return nil, err
	}

	call, ok := expr.(*Call)
	if !ok {
		return nil, p.syntaxError(fmt.Sprintf("expected call expression in do block, got %s", exprDescription(expr)))
	}

	return &Do{Call: call, CallSpan: call.span}, nil
}

func exprDescription(e Expr) string {
	switch e.(type) {
	case *Var:
		return "variable"
	case *Const:
		return "constant"
	case *Call:
		return "call"
	case *List:
		return "list literal"
	case *Map:
		return "map literal"
	case *Test:
		return "test expression"
	case *Filter:
		return "filter expression"
	default:
		return "expression"
	}
}

func (p *Parser) subparse(endCheck func(lexer.Token) bool) ([]Stmt, *Error) {
	var stmts []Stmt

	for {
		tok := p.advance()
		if tok == nil {
			break
		}

		switch tok.Type {
		case lexer.TokenTemplateData:
			stmts = append(stmts, &EmitRaw{Raw: tok.Value, span: tok.Span})

		case lexer.TokenVariableStart:
			span := tok.Span
			expr, err := p.parseExpr()
			if err != nil {
				return nil, err
			}
			stmts = append(stmts, &EmitExpr{Expr: expr, span: p.expandSpan(span)})
			if _, err := p.expect(lexer.TokenVariableEnd, "end of variable block"); err != nil {
				return nil, err
			}

		case lexer.TokenBlockStart:
			if current := p.current(); current == nil {
				return nil, p.syntaxError("unexpected end of input, expected keyword")
			} else if endCheck(*current) {
				return stmts, nil
			}
			stmt, err := p.parseStmt()
			if err != nil {
				return nil, err
			}
			stmts = append(stmts, stmt)
			if _, err := p.expect(lexer.TokenBlockEnd, "end of block"); err != nil {
				return nil, err
			}

		default:
			// This shouldn't happen with well-formed lexer output
			return nil, p.syntaxError(fmt.Sprintf("unexpected token %s", tok.Type))
		}
	}

	return stmts, nil
}

// FormatResult formats a parse result for snapshot testing.
func FormatResult(r Result) string {
	if r.Err != nil {
		return fmt.Sprintf("Err(\n    Error {\n        kind: %s,\n        detail: %q,\n        name: %q,\n        line: %d,\n    },\n)",
			r.Err.Kind, r.Err.Detail, r.Err.Name, r.Err.Line)
	}
	return fmt.Sprintf("Ok(\n    %s,\n)", DebugString(r.Template, 1))
}
