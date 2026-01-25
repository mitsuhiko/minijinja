// Package parser provides parsing for Jinja2 templates.
package parser

import (
	"fmt"
	"math/big"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/lexer"
)

// Span represents a location range in source code.
type Span = lexer.Span

// Node is the interface implemented by all AST nodes.
type Node interface {
	node()
	Span() Span
}

// Stmt represents a statement node.
type Stmt interface {
	Node
	stmt()
}

// Expr represents an expression node.
type Expr interface {
	Node
	expr()
}

// --- Statement Types ---

// Template is the root node of a parsed template.
type Template struct {
	Children []Stmt
	span     Span
}

func (t *Template) node()      {}
func (t *Template) stmt()      {}
func (t *Template) Span() Span { return t.span }

// EmitRaw outputs raw template text.
type EmitRaw struct {
	Raw  string
	span Span
}

func (e *EmitRaw) node()      {}
func (e *EmitRaw) stmt()      {}
func (e *EmitRaw) Span() Span { return e.span }

// EmitExpr outputs an expression result.
type EmitExpr struct {
	Expr Expr
	span Span
}

func (e *EmitExpr) node()      {}
func (e *EmitExpr) stmt()      {}
func (e *EmitExpr) Span() Span { return e.span }

// ForLoop represents a for loop.
type ForLoop struct {
	Target     Expr
	Iter       Expr
	FilterExpr Expr // optional
	Recursive  bool
	Body       []Stmt
	ElseBody   []Stmt
	span       Span
}

func (f *ForLoop) node()      {}
func (f *ForLoop) stmt()      {}
func (f *ForLoop) Span() Span { return f.span }

// IfCond represents an if/elif/else condition.
type IfCond struct {
	Expr      Expr
	TrueBody  []Stmt
	FalseBody []Stmt
	span      Span
}

func (i *IfCond) node()      {}
func (i *IfCond) stmt()      {}
func (i *IfCond) Span() Span { return i.span }

// WithBlock represents a with block.
type WithBlock struct {
	Assignments []Assignment
	Body        []Stmt
	span        Span
}

type Assignment struct {
	Target Expr
	Value  Expr
}

func (w *WithBlock) node()      {}
func (w *WithBlock) stmt()      {}
func (w *WithBlock) Span() Span { return w.span }

// Set represents a variable assignment.
type Set struct {
	Target Expr
	Expr   Expr
	span   Span
}

func (s *Set) node()      {}
func (s *Set) stmt()      {}
func (s *Set) Span() Span { return s.span }

// SetBlock represents a set block (capture).
type SetBlock struct {
	Target Expr
	Filter Expr // optional
	Body   []Stmt
	span   Span
}

func (s *SetBlock) node()      {}
func (s *SetBlock) stmt()      {}
func (s *SetBlock) Span() Span { return s.span }

// AutoEscape represents an autoescape block.
type AutoEscape struct {
	Enabled Expr
	Body    []Stmt
	span    Span
}

func (a *AutoEscape) node()      {}
func (a *AutoEscape) stmt()      {}
func (a *AutoEscape) Span() Span { return a.span }

// FilterBlock represents a filter block.
type FilterBlock struct {
	Filter Expr
	Body   []Stmt
	span   Span
}

func (f *FilterBlock) node()      {}
func (f *FilterBlock) stmt()      {}
func (f *FilterBlock) Span() Span { return f.span }

// Block represents a template block for inheritance.
type Block struct {
	Name string
	Body []Stmt
	span Span
}

func (b *Block) node()      {}
func (b *Block) stmt()      {}
func (b *Block) Span() Span { return b.span }

// Extends represents an extends directive.
type Extends struct {
	Name Expr
	span Span
}

func (e *Extends) node()      {}
func (e *Extends) stmt()      {}
func (e *Extends) Span() Span { return e.span }

// Include represents an include directive.
type Include struct {
	Name          Expr
	IgnoreMissing bool
	span          Span
}

func (i *Include) node()      {}
func (i *Include) stmt()      {}
func (i *Include) Span() Span { return i.span }

// Import represents a full module import.
type Import struct {
	Expr Expr
	Name Expr
	span Span
}

func (i *Import) node()      {}
func (i *Import) stmt()      {}
func (i *Import) Span() Span { return i.span }

// FromImport represents a from ... import statement.
type FromImport struct {
	Expr  Expr
	Names []ImportName
	span  Span
}

type ImportName struct {
	Name  Expr
	Alias Expr // optional
}

func (f *FromImport) node()      {}
func (f *FromImport) stmt()      {}
func (f *FromImport) Span() Span { return f.span }

// Macro represents a macro definition.
type Macro struct {
	Name     string
	Args     []Expr
	Defaults []Expr
	Body     []Stmt
	span     Span
}

func (m *Macro) node()      {}
func (m *Macro) stmt()      {}
func (m *Macro) Span() Span { return m.span }

// CallBlock represents a call block.
type CallBlock struct {
	Call      *Call
	CallSpan  Span
	MacroDecl *Macro
	MacroSpan Span
	span      Span
}

func (c *CallBlock) node()      {}
func (c *CallBlock) stmt()      {}
func (c *CallBlock) Span() Span { return c.span }

// Do represents a do statement.
type Do struct {
	Call     *Call
	CallSpan Span
	span     Span
}

func (d *Do) node()      {}
func (d *Do) stmt()      {}
func (d *Do) Span() Span { return d.span }

// Continue represents a continue statement.
type Continue struct {
	span Span
}

func (c *Continue) node()      {}
func (c *Continue) stmt()      {}
func (c *Continue) Span() Span { return c.span }

// Break represents a break statement.
type Break struct {
	span Span
}

func (b *Break) node()      {}
func (b *Break) stmt()      {}
func (b *Break) Span() Span { return b.span }

// --- Expression Types ---

// Var represents a variable reference.
type Var struct {
	ID   string
	span Span
}

func (v *Var) node()      {}
func (v *Var) expr()      {}
func (v *Var) Span() Span { return v.span }

// Const represents a constant value.
type Const struct {
	Value interface{} // string, int64, float64, bool, or nil
	span  Span
}

func (c *Const) node()      {}
func (c *Const) expr()      {}
func (c *Const) Span() Span { return c.span }

// UnaryOpKind represents the type of unary operator.
type UnaryOpKind int

const (
	UnaryNot UnaryOpKind = iota
	UnaryNeg
)

func (k UnaryOpKind) String() string {
	switch k {
	case UnaryNot:
		return "Not"
	case UnaryNeg:
		return "Neg"
	}
	return "?"
}

// UnaryOp represents a unary operation.
type UnaryOp struct {
	Op   UnaryOpKind
	Expr Expr
	span Span
}

func (u *UnaryOp) node()      {}
func (u *UnaryOp) expr()      {}
func (u *UnaryOp) Span() Span { return u.span }

// BinOpKind represents the type of binary operator.
type BinOpKind int

const (
	BinOpEq BinOpKind = iota
	BinOpNe
	BinOpLt
	BinOpLte
	BinOpGt
	BinOpGte
	BinOpScAnd
	BinOpScOr
	BinOpAdd
	BinOpSub
	BinOpMul
	BinOpDiv
	BinOpFloorDiv
	BinOpRem
	BinOpPow
	BinOpConcat
	BinOpIn
)

func (k BinOpKind) String() string {
	switch k {
	case BinOpEq:
		return "Eq"
	case BinOpNe:
		return "Ne"
	case BinOpLt:
		return "Lt"
	case BinOpLte:
		return "Lte"
	case BinOpGt:
		return "Gt"
	case BinOpGte:
		return "Gte"
	case BinOpScAnd:
		return "ScAnd"
	case BinOpScOr:
		return "ScOr"
	case BinOpAdd:
		return "Add"
	case BinOpSub:
		return "Sub"
	case BinOpMul:
		return "Mul"
	case BinOpDiv:
		return "Div"
	case BinOpFloorDiv:
		return "FloorDiv"
	case BinOpRem:
		return "Rem"
	case BinOpPow:
		return "Pow"
	case BinOpConcat:
		return "Concat"
	case BinOpIn:
		return "In"
	}
	return "?"
}

// BinOp represents a binary operation.
type BinOp struct {
	Op    BinOpKind
	Left  Expr
	Right Expr
	span  Span
}

func (b *BinOp) node()      {}
func (b *BinOp) expr()      {}
func (b *BinOp) Span() Span { return b.span }

// IfExpr represents a conditional expression (ternary).
type IfExpr struct {
	TestExpr  Expr
	TrueExpr  Expr
	FalseExpr Expr // optional
	span      Span
}

func (i *IfExpr) node()      {}
func (i *IfExpr) expr()      {}
func (i *IfExpr) Span() Span { return i.span }

// Filter represents a filter application.
type Filter struct {
	Name string
	Expr Expr // optional (nil for filter chains in set blocks)
	Args []CallArg
	span Span
}

func (f *Filter) node()      {}
func (f *Filter) expr()      {}
func (f *Filter) Span() Span { return f.span }

// Test represents a test expression.
type Test struct {
	Name string
	Expr Expr
	Args []CallArg
	span Span
}

func (t *Test) node()      {}
func (t *Test) expr()      {}
func (t *Test) Span() Span { return t.span }

// GetAttr represents attribute access (x.y).
type GetAttr struct {
	Expr Expr
	Name string
	span Span
}

func (g *GetAttr) node()      {}
func (g *GetAttr) expr()      {}
func (g *GetAttr) Span() Span { return g.span }

// GetItem represents subscript access (x[y]).
type GetItem struct {
	Expr          Expr
	SubscriptExpr Expr
	span          Span
}

func (g *GetItem) node()      {}
func (g *GetItem) expr()      {}
func (g *GetItem) Span() Span { return g.span }

// Slice represents a slice operation.
type Slice struct {
	Expr  Expr
	Start Expr // optional
	Stop  Expr // optional
	Step  Expr // optional
	span  Span
}

func (s *Slice) node()      {}
func (s *Slice) expr()      {}
func (s *Slice) Span() Span { return s.span }

// Call represents a function/method call.
type Call struct {
	Expr Expr
	Args []CallArg
	span Span
}

func (c *Call) node()      {}
func (c *Call) expr()      {}
func (c *Call) Span() Span { return c.span }

// CallArgKind represents the type of call argument.
type CallArgKind int

const (
	CallArgPos CallArgKind = iota
	CallArgKwarg
	CallArgPosSplat
	CallArgKwargSplat
)

// CallArg represents a function call argument.
type CallArg struct {
	Kind  CallArgKind
	Name  string // for kwargs
	Value Expr
}

// List represents a list literal.
type List struct {
	Items []Expr
	span  Span
}

func (l *List) node()      {}
func (l *List) expr()      {}
func (l *List) Span() Span { return l.span }

// Map represents a map/dict literal.
type Map struct {
	Keys   []Expr
	Values []Expr
	span   Span
}

func (m *Map) node()      {}
func (m *Map) expr()      {}
func (m *Map) Span() Span { return m.span }

// --- Debug Output (matching Rust's format) ---

// FormatSpan formats a span like Rust does: " @ line:col-line:col"
func FormatSpan(s Span) string {
	return fmt.Sprintf(" @ %d:%d-%d:%d", s.StartLine, s.StartCol, s.EndLine, s.EndCol)
}

// DebugString returns a Rust-like debug representation of a node.
func DebugString(n Node, indent int) string {
	ind := strings.Repeat("    ", indent)
	ind1 := strings.Repeat("    ", indent+1)
	ind2 := strings.Repeat("    ", indent+2)
	_ = ind2 // may be unused

	switch v := n.(type) {
	case *Template:
		var sb strings.Builder
		sb.WriteString("Template {\n")
		sb.WriteString(ind1)
		sb.WriteString("children: ")
		sb.WriteString(debugStmtList(v.Children, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *EmitRaw:
		return fmt.Sprintf("EmitRaw {\n%sraw: %q,\n%s}%s", ind1, v.Raw, ind, FormatSpan(v.span))

	case *EmitExpr:
		var sb strings.Builder
		sb.WriteString("EmitExpr {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *ForLoop:
		var sb strings.Builder
		sb.WriteString("ForLoop {\n")
		sb.WriteString(ind1)
		sb.WriteString("target: ")
		sb.WriteString(DebugString(v.Target, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("iter: ")
		sb.WriteString(DebugString(v.Iter, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("filter_expr: ")
		if v.FilterExpr != nil {
			sb.WriteString("Some(\n")
			sb.WriteString(ind2)
			sb.WriteString(DebugString(v.FilterExpr, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		} else {
			sb.WriteString("None")
		}
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("recursive: %v,\n", v.Recursive))
		sb.WriteString(ind1)
		sb.WriteString("body: ")
		sb.WriteString(debugStmtList(v.Body, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("else_body: ")
		sb.WriteString(debugStmtList(v.ElseBody, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *IfCond:
		var sb strings.Builder
		sb.WriteString("IfCond {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("true_body: ")
		sb.WriteString(debugStmtList(v.TrueBody, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("false_body: ")
		sb.WriteString(debugStmtList(v.FalseBody, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *WithBlock:
		var sb strings.Builder
		sb.WriteString("WithBlock {\n")
		sb.WriteString(ind1)
		sb.WriteString("assignments: [\n")
		for _, a := range v.Assignments {
			sb.WriteString(ind2)
			sb.WriteString("(\n")
			sb.WriteString(strings.Repeat("    ", indent+3))
			sb.WriteString(DebugString(a.Target, indent+3))
			sb.WriteString(",\n")
			sb.WriteString(strings.Repeat("    ", indent+3))
			sb.WriteString(DebugString(a.Value, indent+3))
			sb.WriteString(",\n")
			sb.WriteString(ind2)
			sb.WriteString("),\n")
		}
		sb.WriteString(ind1)
		sb.WriteString("],\n")
		sb.WriteString(ind1)
		sb.WriteString("body: ")
		sb.WriteString(debugStmtList(v.Body, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Set:
		var sb strings.Builder
		sb.WriteString("Set {\n")
		sb.WriteString(ind1)
		sb.WriteString("target: ")
		sb.WriteString(DebugString(v.Target, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *SetBlock:
		var sb strings.Builder
		sb.WriteString("SetBlock {\n")
		sb.WriteString(ind1)
		sb.WriteString("target: ")
		sb.WriteString(DebugString(v.Target, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("filter: ")
		if v.Filter != nil {
			sb.WriteString("Some(\n")
			sb.WriteString(ind2)
			sb.WriteString(DebugString(v.Filter, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		} else {
			sb.WriteString("None")
		}
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("body: ")
		sb.WriteString(debugStmtList(v.Body, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *AutoEscape:
		var sb strings.Builder
		sb.WriteString("AutoEscape {\n")
		sb.WriteString(ind1)
		sb.WriteString("enabled: ")
		sb.WriteString(DebugString(v.Enabled, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("body: ")
		sb.WriteString(debugStmtList(v.Body, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *FilterBlock:
		var sb strings.Builder
		sb.WriteString("FilterBlock {\n")
		sb.WriteString(ind1)
		sb.WriteString("filter: ")
		sb.WriteString(DebugString(v.Filter, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("body: ")
		sb.WriteString(debugStmtList(v.Body, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Block:
		var sb strings.Builder
		sb.WriteString("Block {\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("name: %q,\n", v.Name))
		sb.WriteString(ind1)
		sb.WriteString("body: ")
		sb.WriteString(debugStmtList(v.Body, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Extends:
		var sb strings.Builder
		sb.WriteString("Extends {\n")
		sb.WriteString(ind1)
		sb.WriteString("name: ")
		sb.WriteString(DebugString(v.Name, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Include:
		var sb strings.Builder
		sb.WriteString("Include {\n")
		sb.WriteString(ind1)
		sb.WriteString("name: ")
		sb.WriteString(DebugString(v.Name, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("ignore_missing: %v,\n", v.IgnoreMissing))
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Import:
		var sb strings.Builder
		sb.WriteString("Import {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("name: ")
		sb.WriteString(DebugString(v.Name, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *FromImport:
		var sb strings.Builder
		sb.WriteString("FromImport {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("names: [\n")
		for _, n := range v.Names {
			sb.WriteString(ind2)
			sb.WriteString("(\n")
			sb.WriteString(strings.Repeat("    ", indent+3))
			sb.WriteString(DebugString(n.Name, indent+3))
			sb.WriteString(",\n")
			sb.WriteString(strings.Repeat("    ", indent+3))
			if n.Alias != nil {
				sb.WriteString("Some(\n")
				sb.WriteString(strings.Repeat("    ", indent+4))
				sb.WriteString(DebugString(n.Alias, indent+4))
				sb.WriteString(",\n")
				sb.WriteString(strings.Repeat("    ", indent+3))
				sb.WriteString(")")
			} else {
				sb.WriteString("None")
			}
			sb.WriteString(",\n")
			sb.WriteString(ind2)
			sb.WriteString("),\n")
		}
		sb.WriteString(ind1)
		sb.WriteString("],\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Macro:
		var sb strings.Builder
		sb.WriteString("Macro {\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("name: %q,\n", v.Name))
		sb.WriteString(ind1)
		sb.WriteString("args: ")
		sb.WriteString(debugExprList(v.Args, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("defaults: ")
		sb.WriteString(debugExprList(v.Defaults, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("body: ")
		sb.WriteString(debugStmtList(v.Body, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *CallBlock:
		var sb strings.Builder
		sb.WriteString("CallBlock {\n")
		sb.WriteString(ind1)
		sb.WriteString("call: ")
		sb.WriteString(debugCallInner(v.Call, indent+1))
		sb.WriteString(FormatSpan(v.CallSpan))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("macro_decl: ")
		sb.WriteString(DebugString(v.MacroDecl, indent+1))
		// Use MacroSpan for macro_decl
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Do:
		var sb strings.Builder
		sb.WriteString("Do {\n")
		sb.WriteString(ind1)
		sb.WriteString("call: ")
		sb.WriteString(debugCallInner(v.Call, indent+1))
		sb.WriteString(FormatSpan(v.CallSpan))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Continue:
		return "Continue" + FormatSpan(v.span)

	case *Break:
		return "Break" + FormatSpan(v.span)

	case *Var:
		return fmt.Sprintf("Var {\n%sid: %q,\n%s}%s", ind1, v.ID, ind, FormatSpan(v.span))

	case *Const:
		return fmt.Sprintf("Const {\n%svalue: %s,\n%s}%s", ind1, formatValue(v.Value), ind, FormatSpan(v.span))

	case *UnaryOp:
		var sb strings.Builder
		sb.WriteString("UnaryOp {\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("op: %s,\n", v.Op))
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *BinOp:
		var sb strings.Builder
		sb.WriteString("BinOp {\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("op: %s,\n", v.Op))
		sb.WriteString(ind1)
		sb.WriteString("left: ")
		sb.WriteString(DebugString(v.Left, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("right: ")
		sb.WriteString(DebugString(v.Right, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *IfExpr:
		var sb strings.Builder
		sb.WriteString("IfExpr {\n")
		sb.WriteString(ind1)
		sb.WriteString("test_expr: ")
		sb.WriteString(DebugString(v.TestExpr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("true_expr: ")
		sb.WriteString(DebugString(v.TrueExpr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("false_expr: ")
		if v.FalseExpr != nil {
			sb.WriteString("Some(\n")
			sb.WriteString(ind2)
			sb.WriteString(DebugString(v.FalseExpr, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		} else {
			sb.WriteString("None")
		}
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Filter:
		var sb strings.Builder
		sb.WriteString("Filter {\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("name: %q,\n", v.Name))
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		if v.Expr != nil {
			sb.WriteString("Some(\n")
			sb.WriteString(ind2)
			sb.WriteString(DebugString(v.Expr, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		} else {
			sb.WriteString("None")
		}
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("args: ")
		sb.WriteString(debugCallArgs(v.Args, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Test:
		var sb strings.Builder
		sb.WriteString("Test {\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("name: %q,\n", v.Name))
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("args: ")
		sb.WriteString(debugCallArgs(v.Args, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *GetAttr:
		var sb strings.Builder
		sb.WriteString("GetAttr {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString(fmt.Sprintf("name: %q,\n", v.Name))
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *GetItem:
		var sb strings.Builder
		sb.WriteString("GetItem {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("subscript_expr: ")
		sb.WriteString(DebugString(v.SubscriptExpr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Slice:
		var sb strings.Builder
		sb.WriteString("Slice {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("start: ")
		sb.WriteString(debugOptionalExpr(v.Start, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("stop: ")
		sb.WriteString(debugOptionalExpr(v.Stop, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("step: ")
		sb.WriteString(debugOptionalExpr(v.Step, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Call:
		var sb strings.Builder
		sb.WriteString("Call {\n")
		sb.WriteString(ind1)
		sb.WriteString("expr: ")
		sb.WriteString(DebugString(v.Expr, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("args: ")
		sb.WriteString(debugCallArgs(v.Args, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *List:
		var sb strings.Builder
		sb.WriteString("List {\n")
		sb.WriteString(ind1)
		sb.WriteString("items: ")
		sb.WriteString(debugExprList(v.Items, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	case *Map:
		var sb strings.Builder
		sb.WriteString("Map {\n")
		sb.WriteString(ind1)
		sb.WriteString("keys: ")
		sb.WriteString(debugExprList(v.Keys, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind1)
		sb.WriteString("values: ")
		sb.WriteString(debugExprList(v.Values, indent+1))
		sb.WriteString(",\n")
		sb.WriteString(ind)
		sb.WriteString("}")
		sb.WriteString(FormatSpan(v.span))
		return sb.String()

	default:
		return fmt.Sprintf("<%T>", n)
	}
}

func debugCallInner(c *Call, indent int) string {
	ind1 := strings.Repeat("    ", indent+1)
	ind := strings.Repeat("    ", indent)

	var sb strings.Builder
	sb.WriteString("Call {\n")
	sb.WriteString(ind1)
	sb.WriteString("expr: ")
	sb.WriteString(DebugString(c.Expr, indent+1))
	sb.WriteString(",\n")
	sb.WriteString(ind1)
	sb.WriteString("args: ")
	sb.WriteString(debugCallArgs(c.Args, indent+1))
	sb.WriteString(",\n")
	sb.WriteString(ind)
	sb.WriteString("}")
	return sb.String()
}

func formatValue(v interface{}) string {
	switch val := v.(type) {
	case nil:
		return "()"
	case bool:
		return fmt.Sprintf("%v", val)
	case string:
		return fmt.Sprintf("%q", val)
	case int64:
		return fmt.Sprintf("%d", val)
	case float64:
		// Rust formats floats without trailing zero if integer
		if val == float64(int64(val)) {
			return fmt.Sprintf("%.1f", val)
		}
		return fmt.Sprintf("%v", val)
	case *BigInt:
		return val.String()
	default:
		return fmt.Sprintf("%v", v)
	}
}

// BigInt wraps big.Int for large integer constants
type BigInt struct {
	*big.Int
}

func (b *BigInt) String() string {
	return b.Int.String()
}

func debugStmtList(stmts []Stmt, indent int) string {
	if len(stmts) == 0 {
		return "[]"
	}
	ind1 := strings.Repeat("    ", indent+1)
	ind := strings.Repeat("    ", indent)
	var sb strings.Builder
	sb.WriteString("[\n")
	for _, s := range stmts {
		sb.WriteString(ind1)
		sb.WriteString(DebugString(s, indent+1))
		sb.WriteString(",\n")
	}
	sb.WriteString(ind)
	sb.WriteString("]")
	return sb.String()
}

func debugExprList(exprs []Expr, indent int) string {
	if len(exprs) == 0 {
		return "[]"
	}
	ind1 := strings.Repeat("    ", indent+1)
	ind := strings.Repeat("    ", indent)
	var sb strings.Builder
	sb.WriteString("[\n")
	for _, e := range exprs {
		sb.WriteString(ind1)
		sb.WriteString(DebugString(e, indent+1))
		sb.WriteString(",\n")
	}
	sb.WriteString(ind)
	sb.WriteString("]")
	return sb.String()
}

func debugCallArgs(args []CallArg, indent int) string {
	if len(args) == 0 {
		return "[]"
	}
	ind1 := strings.Repeat("    ", indent+1)
	ind := strings.Repeat("    ", indent)
	var sb strings.Builder
	sb.WriteString("[\n")
	for _, a := range args {
		sb.WriteString(ind1)
		switch a.Kind {
		case CallArgPos:
			sb.WriteString("Pos(\n")
			sb.WriteString(strings.Repeat("    ", indent+2))
			sb.WriteString(DebugString(a.Value, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		case CallArgKwarg:
			sb.WriteString(fmt.Sprintf("Kwarg(\n%s%q,\n", strings.Repeat("    ", indent+2), a.Name))
			sb.WriteString(strings.Repeat("    ", indent+2))
			sb.WriteString(DebugString(a.Value, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		case CallArgPosSplat:
			sb.WriteString("PosSplat(\n")
			sb.WriteString(strings.Repeat("    ", indent+2))
			sb.WriteString(DebugString(a.Value, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		case CallArgKwargSplat:
			sb.WriteString("KwargSplat(\n")
			sb.WriteString(strings.Repeat("    ", indent+2))
			sb.WriteString(DebugString(a.Value, indent+2))
			sb.WriteString(",\n")
			sb.WriteString(ind1)
			sb.WriteString(")")
		}
		sb.WriteString(",\n")
	}
	sb.WriteString(ind)
	sb.WriteString("]")
	return sb.String()
}

func debugOptionalExpr(e Expr, indent int) string {
	if e == nil {
		return "None"
	}
	ind1 := strings.Repeat("    ", indent+1)
	ind := strings.Repeat("    ", indent)
	var sb strings.Builder
	sb.WriteString("Some(\n")
	sb.WriteString(ind1)
	sb.WriteString(DebugString(e, indent+1))
	sb.WriteString(",\n")
	sb.WriteString(ind)
	sb.WriteString(")")
	return sb.String()
}
