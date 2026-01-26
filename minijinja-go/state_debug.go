package minijinja

import (
	mjerrors "github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/errors"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/parser"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func (s *State) attachErrorInfo(err error, node parser.Node) error {
	if err == nil || s.env == nil || !s.env.debug {
		return err
	}
	templErr, ok := err.(*Error)
	if !ok {
		return err
	}
	if templErr.Name == "" {
		templErr.WithName(s.name)
	}
	if templErr.Source == "" {
		templErr.WithSource(s.source)
	}
	if templErr.Span == nil && node != nil {
		templErr.WithSpan(node.Span())
	}
	if templErr.DebugInfo == nil {
		templErr.WithDebugInfo(s.makeDebugInfo(node))
	}
	return err
}

func (s *State) makeDebugInfo(node parser.Node) mjerrors.DebugInfo {
	referenced := map[string]struct{}{}
	if node != nil {
		switch typed := node.(type) {
		case parser.Expr:
			collectReferencedNamesExpr(typed, referenced)
		case parser.Stmt:
			collectReferencedNamesStmt(typed, referenced)
		}
	}

	locals := make(map[string]value.Value, len(referenced))
	for name := range referenced {
		val := s.Lookup(name)
		if !val.IsUndefined() {
			locals[name] = val
		}
	}

	return mjerrors.DebugInfo{
		TemplateSource:   s.source,
		ReferencedLocals: locals,
	}
}

func collectReferencedNamesStmt(stmt parser.Stmt, referenced map[string]struct{}) {
	switch s := stmt.(type) {
	case *parser.EmitExpr:
		collectReferencedNamesExpr(s.Expr, referenced)
	case *parser.ForLoop:
		collectReferencedNamesExpr(s.Iter, referenced)
		if s.FilterExpr != nil {
			collectReferencedNamesExpr(s.FilterExpr, referenced)
		}
	case *parser.IfCond:
		collectReferencedNamesExpr(s.Expr, referenced)
	case *parser.WithBlock:
		for _, assignment := range s.Assignments {
			collectReferencedNamesExpr(assignment.Value, referenced)
		}
	case *parser.Set:
		collectReferencedNamesExpr(s.Expr, referenced)
	case *parser.SetBlock:
		if s.Filter != nil {
			collectReferencedNamesExpr(s.Filter, referenced)
		}
	case *parser.AutoEscape:
		collectReferencedNamesExpr(s.Enabled, referenced)
	case *parser.FilterBlock:
		collectReferencedNamesExpr(s.Filter, referenced)
	case *parser.Extends:
		collectReferencedNamesExpr(s.Name, referenced)
	case *parser.Include:
		collectReferencedNamesExpr(s.Name, referenced)
	case *parser.Import:
		collectReferencedNamesExpr(s.Expr, referenced)
	case *parser.FromImport:
		collectReferencedNamesExpr(s.Expr, referenced)
	case *parser.CallBlock:
		if s.Call != nil {
			collectReferencedNamesExpr(s.Call, referenced)
		}
	case *parser.Do:
		collectReferencedNamesExpr(s.Call, referenced)
	}
}

func collectReferencedNamesExpr(expr parser.Expr, referenced map[string]struct{}) {
	if expr == nil {
		return
	}

	switch e := expr.(type) {
	case *parser.Var:
		referenced[e.ID] = struct{}{}
	case *parser.Const:
		return
	case *parser.UnaryOp:
		collectReferencedNamesExpr(e.Expr, referenced)
	case *parser.BinOp:
		collectReferencedNamesExpr(e.Left, referenced)
		collectReferencedNamesExpr(e.Right, referenced)
	case *parser.IfExpr:
		collectReferencedNamesExpr(e.TestExpr, referenced)
		collectReferencedNamesExpr(e.TrueExpr, referenced)
		if e.FalseExpr != nil {
			collectReferencedNamesExpr(e.FalseExpr, referenced)
		}
	case *parser.Filter:
		if e.Expr != nil {
			collectReferencedNamesExpr(e.Expr, referenced)
		}
		collectReferencedNamesCallArgs(e.Args, referenced)
	case *parser.Test:
		collectReferencedNamesExpr(e.Expr, referenced)
		collectReferencedNamesCallArgs(e.Args, referenced)
	case *parser.GetAttr:
		collectReferencedNamesExpr(e.Expr, referenced)
	case *parser.GetItem:
		collectReferencedNamesExpr(e.Expr, referenced)
		collectReferencedNamesExpr(e.SubscriptExpr, referenced)
	case *parser.Slice:
		collectReferencedNamesExpr(e.Expr, referenced)
		collectReferencedNamesExpr(e.Start, referenced)
		collectReferencedNamesExpr(e.Stop, referenced)
		collectReferencedNamesExpr(e.Step, referenced)
	case *parser.Call:
		collectReferencedNamesExpr(e.Expr, referenced)
		collectReferencedNamesCallArgs(e.Args, referenced)
	case *parser.List:
		for _, item := range e.Items {
			collectReferencedNamesExpr(item, referenced)
		}
	case *parser.Map:
		for _, key := range e.Keys {
			collectReferencedNamesExpr(key, referenced)
		}
		for _, value := range e.Values {
			collectReferencedNamesExpr(value, referenced)
		}
	}
}

func collectReferencedNamesCallArgs(args []parser.CallArg, referenced map[string]struct{}) {
	for _, arg := range args {
		collectReferencedNamesExpr(arg.Value, referenced)
	}
}
