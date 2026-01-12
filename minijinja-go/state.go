package minijinja

import (
	"fmt"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/parser"
	"github.com/mitsuhiko/minijinja/minijinja-go/value"
)

// State holds the evaluation state during template rendering.
type State struct {
	env          *Environment
	name         string
	source       string
	autoEscape   AutoEscape
	scopes       []map[string]value.Value
	blocks       map[string]*blockStack
	macros       map[string]*parser.Macro
	out          *strings.Builder
	depth        int
	currentBlock string              // name of block currently being rendered
	loopRecurse  func(value.Value) (string, error) // for recursive loops
}

// blockStack manages the inheritance chain for a single block
type blockStack struct {
	layers [][]parser.Stmt // stack of block implementations (child first)
	index  int             // current index in stack
}

// macroCallable wraps a macro for callable invocation
type macroCallable struct {
	macro *parser.Macro
	state *State
}

func (m *macroCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	// Convert args to CallArgs format
	callArgs := make([]parser.CallArg, 0, len(args)+len(kwargs))
	for _, arg := range args {
		callArgs = append(callArgs, parser.CallArg{
			Kind:  parser.CallArgPos,
			Value: &parser.Const{Value: arg.Raw()},
		})
	}
	for name, val := range kwargs {
		callArgs = append(callArgs, parser.CallArg{
			Kind:  parser.CallArgKwarg,
			Name:  name,
			Value: &parser.Const{Value: val.Raw()},
		})
	}

	return m.state.callMacroWithValues(m.macro, args, kwargs)
}

func (s *State) callMacroWithValues(macro *parser.Macro, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	s.pushScope()
	defer s.popScope()

	// Bind arguments
	for i, arg := range macro.Args {
		if varArg, ok := arg.(*parser.Var); ok {
			// Check if provided as kwarg
			if val, ok := kwargs[varArg.ID]; ok {
				s.Set(varArg.ID, val)
				continue
			}
			// Check if provided as positional arg
			if i < len(args) {
				s.Set(varArg.ID, args[i])
			} else if i-len(macro.Args)+len(macro.Defaults) >= 0 {
				// Use default value
				defaultIdx := i - len(macro.Args) + len(macro.Defaults)
				if defaultIdx >= 0 && defaultIdx < len(macro.Defaults) {
					val, err := s.evalExpr(macro.Defaults[defaultIdx])
					if err != nil {
						return value.Undefined(), err
					}
					s.Set(varArg.ID, val)
				} else {
					s.Set(varArg.ID, value.Undefined())
				}
			} else {
				s.Set(varArg.ID, value.Undefined())
			}
		}
	}

	// Capture output
	oldOut := s.out
	s.out = &strings.Builder{}
	for _, stmt := range macro.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return value.Undefined(), err
		}
	}
	result := s.out.String()
	s.out = oldOut

	return value.FromSafeString(result), nil
}

// LoopState holds information about the current loop iteration.
type LoopState struct {
	Index     int   // 1-based index
	Index0    int   // 0-based index
	RevIndex  int   // reverse 1-based index
	RevIndex0 int   // reverse 0-based index
	First     bool  // is first iteration
	Last      bool  // is last iteration
	Length    int   // total length
	Depth     int   // nesting depth (1-based)
	Depth0    int   // nesting depth (0-based)
	Cycle     []value.Value // cycle values
}

// ToValue converts LoopState to a Value.
func (l *LoopState) ToValue() value.Value {
	m := map[string]value.Value{
		"index":     value.FromInt(int64(l.Index)),
		"index0":    value.FromInt(int64(l.Index0)),
		"revindex":  value.FromInt(int64(l.RevIndex)),
		"revindex0": value.FromInt(int64(l.RevIndex0)),
		"first":     value.FromBool(l.First),
		"last":      value.FromBool(l.Last),
		"length":    value.FromInt(int64(l.Length)),
		"depth":     value.FromInt(int64(l.Depth)),
		"depth0":    value.FromInt(int64(l.Depth0)),
	}
	return value.FromMap(m)
}

const maxRecursion = 500

func newState(env *Environment, name, source string, ctx value.Value) *State {
	// Initialize root scope with context
	rootScope := make(map[string]value.Value)
	if m, ok := ctx.AsMap(); ok {
		for k, v := range m {
			rootScope[k] = v
		}
	}

	return &State{
		env:        env,
		name:       name,
		source:     source,
		autoEscape: env.autoEscapeFunc(name),
		scopes:     []map[string]value.Value{rootScope},
		blocks:     make(map[string]*blockStack),
		macros:     make(map[string]*parser.Macro),
		out:        &strings.Builder{},
	}
}

// Lookup looks up a variable in the current scope chain.
func (s *State) Lookup(name string) value.Value {
	// Search scopes from inner to outer
	for i := len(s.scopes) - 1; i >= 0; i-- {
		if v, ok := s.scopes[i][name]; ok {
			return v
		}
	}

	// Check globals
	if v, ok := s.env.getGlobal(name); ok {
		return v
	}

	return value.Undefined()
}

// Set sets a variable in the current scope.
func (s *State) Set(name string, val value.Value) {
	s.scopes[len(s.scopes)-1][name] = val
}

// pushScope creates a new scope.
func (s *State) pushScope() {
	s.scopes = append(s.scopes, make(map[string]value.Value))
}

// popScope removes the current scope.
func (s *State) popScope() {
	if len(s.scopes) > 1 {
		s.scopes = s.scopes[:len(s.scopes)-1]
	}
}

// eval evaluates a template AST.
func (s *State) eval(tmpl *parser.Template) (string, error) {
	// First, check if this template extends another
	// If so, collect all blocks first, then process extends
	var extendsStmt *parser.Extends
	for _, stmt := range tmpl.Children {
		if ext, ok := stmt.(*parser.Extends); ok {
			extendsStmt = ext
			break
		}
	}

	if extendsStmt != nil {
		// Collect all blocks from this (child) template first
		for _, stmt := range tmpl.Children {
			if block, ok := stmt.(*parser.Block); ok {
				s.blocks[block.Name] = &blockStack{
					layers: [][]parser.Stmt{block.Body},
					index:  0,
				}
			}
			// Also process macros
			if macro, ok := stmt.(*parser.Macro); ok {
				s.macros[macro.Name] = macro
			}
		}
		// Now process extends
		if err := s.evalExtends(extendsStmt); err != nil && err != errExtendsExecuted {
			return "", err
		}
		return s.out.String(), nil
	}

	// Non-extending template - evaluate normally
	for _, stmt := range tmpl.Children {
		if err := s.evalStmt(stmt); err != nil {
			return "", err
		}
	}
	return s.out.String(), nil
}

func (s *State) evalStmt(stmt parser.Stmt) error {
	switch st := stmt.(type) {
	case *parser.EmitRaw:
		s.out.WriteString(st.Raw)
		return nil

	case *parser.EmitExpr:
		val, err := s.evalExpr(st.Expr)
		if err != nil {
			return err
		}
		s.writeValue(val)
		return nil

	case *parser.ForLoop:
		return s.evalForLoop(st)

	case *parser.IfCond:
		return s.evalIfCond(st)

	case *parser.WithBlock:
		return s.evalWithBlock(st)

	case *parser.Set:
		return s.evalSet(st)

	case *parser.SetBlock:
		return s.evalSetBlock(st)

	case *parser.Block:
		return s.evalBlock(st)

	case *parser.Extends:
		return s.evalExtends(st)

	case *parser.Import:
		return s.evalImport(st)

	case *parser.FromImport:
		return s.evalFromImport(st)

	case *parser.Include:
		return s.evalInclude(st)

	case *parser.Macro:
		s.macros[st.Name] = st
		return nil

	case *parser.FilterBlock:
		return s.evalFilterBlock(st)

	case *parser.AutoEscape:
		return s.evalAutoEscape(st)

	case *parser.Do:
		_, err := s.evalCall(st.Call)
		return err

	case *parser.Continue:
		return errContinue

	case *parser.Break:
		return errBreak

	default:
		return fmt.Errorf("unsupported statement type: %T", stmt)
	}
}

// sentinel errors for loop control
var (
	errContinue = fmt.Errorf("continue")
	errBreak    = fmt.Errorf("break")
)

func (s *State) evalForLoop(loop *parser.ForLoop) error {
	iter, err := s.evalExpr(loop.Iter)
	if err != nil {
		return err
	}

	items := iter.Iter()
	if items == nil {
		// Not iterable, execute else body
		if loop.ElseBody != nil {
			for _, stmt := range loop.ElseBody {
				if err := s.evalStmt(stmt); err != nil {
					return err
				}
			}
		}
		return nil
	}

	// Apply filter if present
	if loop.FilterExpr != nil {
		filtered := make([]value.Value, 0, len(items))
		s.pushScope()
		for _, item := range items {
			s.unpackTarget(loop.Target, item)
			cond, err := s.evalExpr(loop.FilterExpr)
			if err != nil {
				s.popScope()
				return err
			}
			if cond.IsTrue() {
				filtered = append(filtered, item)
			}
		}
		s.popScope()
		items = filtered
	}

	if len(items) == 0 {
		// Execute else body
		if loop.ElseBody != nil {
			for _, stmt := range loop.ElseBody {
				if err := s.evalStmt(stmt); err != nil {
					return err
				}
			}
		}
		return nil
	}

	s.depth++
	if s.depth > maxRecursion {
		return NewError(ErrInvalidOperation, "recursion limit exceeded")
	}

	s.pushScope()
	defer func() {
		s.popScope()
		s.depth--
	}()

	// Set up recursive loop function if needed
	var oldRecurse func(value.Value) (string, error)
	if loop.Recursive {
		oldRecurse = s.loopRecurse
		s.loopRecurse = func(iterValue value.Value) (string, error) {
			nestedItems := iterValue.Iter()
			if nestedItems == nil {
				return "", nil
			}

			oldOut := s.out
			s.out = &strings.Builder{}
			
			for i, item := range nestedItems {
				s.unpackTarget(loop.Target, item)
				
				loopState := &LoopState{
					Index:     i + 1,
					Index0:    i,
					RevIndex:  len(nestedItems) - i,
					RevIndex0: len(nestedItems) - i - 1,
					First:     i == 0,
					Last:      i == len(nestedItems)-1,
					Length:    len(nestedItems),
					Depth:     s.depth,
					Depth0:    s.depth - 1,
				}
				s.Set("loop", loopState.ToValue())
				
				for _, stmt := range loop.Body {
					err := s.evalStmt(stmt)
					if err == errContinue {
						break
					}
					if err == errBreak {
						result := s.out.String()
						s.out = oldOut
						return result, nil
					}
					if err != nil {
						s.out = oldOut
						return "", err
					}
				}
			}
			
			result := s.out.String()
			s.out = oldOut
			return result, nil
		}
		defer func() { s.loopRecurse = oldRecurse }()
	}

	for i, item := range items {
		s.unpackTarget(loop.Target, item)

		// Set loop variable
		loopState := &LoopState{
			Index:     i + 1,
			Index0:    i,
			RevIndex:  len(items) - i,
			RevIndex0: len(items) - i - 1,
			First:     i == 0,
			Last:      i == len(items)-1,
			Length:    len(items),
			Depth:     s.depth,
			Depth0:    s.depth - 1,
		}
		s.Set("loop", loopState.ToValue())

		for _, stmt := range loop.Body {
			err := s.evalStmt(stmt)
			if err == errContinue {
				break
			}
			if err == errBreak {
				return nil
			}
			if err != nil {
				return err
			}
		}
	}

	return nil
}

func (s *State) unpackTarget(target parser.Expr, val value.Value) {
	switch t := target.(type) {
	case *parser.Var:
		s.Set(t.ID, val)
	case *parser.List:
		if items, ok := val.AsSlice(); ok {
			for i, item := range t.Items {
				if i < len(items) {
					s.unpackTarget(item, items[i])
				} else {
					s.unpackTarget(item, value.Undefined())
				}
			}
		}
	case *parser.GetAttr:
		// Handle attribute assignment (e.g., ns.count = value)
		obj, err := s.evalExpr(t.Expr)
		if err != nil {
			return
		}
		if mutableObj, ok := obj.AsMutableObject(); ok {
			mutableObj.SetAttr(t.Name, val)
		}
	}
}

func (s *State) evalIfCond(cond *parser.IfCond) error {
	val, err := s.evalExpr(cond.Expr)
	if err != nil {
		return err
	}

	if val.IsTrue() {
		for _, stmt := range cond.TrueBody {
			if err := s.evalStmt(stmt); err != nil {
				return err
			}
		}
	} else if cond.FalseBody != nil {
		for _, stmt := range cond.FalseBody {
			if err := s.evalStmt(stmt); err != nil {
				return err
			}
		}
	}
	return nil
}

func (s *State) evalWithBlock(block *parser.WithBlock) error {
	s.pushScope()
	defer s.popScope()

	for _, assign := range block.Assignments {
		val, err := s.evalExpr(assign.Value)
		if err != nil {
			return err
		}
		s.unpackTarget(assign.Target, val)
	}

	for _, stmt := range block.Body {
		if err := s.evalStmt(stmt); err != nil {
			return err
		}
	}
	return nil
}

func (s *State) evalSet(set *parser.Set) error {
	val, err := s.evalExpr(set.Expr)
	if err != nil {
		return err
	}
	s.unpackTarget(set.Target, val)
	return nil
}

func (s *State) evalSetBlock(block *parser.SetBlock) error {
	// Capture output
	oldOut := s.out
	s.out = &strings.Builder{}
	for _, stmt := range block.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return err
		}
	}
	captured := s.out.String()
	s.out = oldOut

	result := value.FromString(captured)

	// Apply filter if present
	if block.Filter != nil {
		var err error
		result, err = s.applyFilter(block.Filter, result)
		if err != nil {
			return err
		}
	}

	s.unpackTarget(block.Target, result)
	return nil
}

func (s *State) evalExtends(ext *parser.Extends) error {
	nameVal, err := s.evalExpr(ext.Name)
	if err != nil {
		return err
	}

	name, ok := nameVal.AsString()
	if !ok {
		return NewError(ErrInvalidOperation, "extends name must be a string")
	}

	// Load the parent template
	parentTmpl, err := s.env.GetTemplate(name)
	if err != nil {
		return err
	}

	s.depth++
	if s.depth > maxRecursion {
		return NewError(ErrInvalidOperation, "recursion limit exceeded")
	}
	defer func() { s.depth-- }()

	// Check if parent also extends another template
	var parentExtendsStmt *parser.Extends
	for _, stmt := range parentTmpl.compiled.ast.Children {
		if ext, ok := stmt.(*parser.Extends); ok {
			parentExtendsStmt = ext
			break
		}
	}

	// Collect parent blocks - add them as fallback layers
	for _, stmt := range parentTmpl.compiled.ast.Children {
		if block, ok := stmt.(*parser.Block); ok {
			if bs, exists := s.blocks[block.Name]; exists {
				// Append parent block to the end (child is at index 0)
				bs.layers = append(bs.layers, block.Body)
			} else {
				// This is a parent-only block (no child override)
				s.blocks[block.Name] = &blockStack{
					layers: [][]parser.Stmt{block.Body},
					index:  0,
				}
			}
		}
		// Also collect macros from parent
		if macro, ok := stmt.(*parser.Macro); ok {
			if _, exists := s.macros[macro.Name]; !exists {
				s.macros[macro.Name] = macro
			}
		}
	}

	// If parent extends another template, process that first
	if parentExtendsStmt != nil {
		if err := s.evalExtends(parentExtendsStmt); err != nil && err != errExtendsExecuted {
			return err
		}
		return errExtendsExecuted
	}

	// Render the root parent template
	for _, stmt := range parentTmpl.compiled.ast.Children {
		// Skip extends (already handled)
		if _, isExtends := stmt.(*parser.Extends); isExtends {
			continue
		}
		if err := s.evalStmt(stmt); err != nil {
			return err
		}
	}

	return errExtendsExecuted
}

// errExtendsExecuted signals that extends was executed
var errExtendsExecuted = fmt.Errorf("extends executed")

func (s *State) evalBlock(block *parser.Block) error {
	// When we encounter a block, render using the block stack
	bs := s.blocks[block.Name]
	if bs == nil || len(bs.layers) == 0 {
		// No override - render the current block's content
		s.pushScope()
		oldBlock := s.currentBlock
		s.currentBlock = block.Name
		for _, stmt := range block.Body {
			if err := s.evalStmt(stmt); err != nil {
				s.popScope()
				s.currentBlock = oldBlock
				return err
			}
		}
		s.popScope()
		s.currentBlock = oldBlock
		return nil
	}

	// Render from the top of the stack (child-most block)
	oldBlock := s.currentBlock
	s.currentBlock = block.Name
	bs.index = 0

	s.pushScope()
	for _, stmt := range bs.layers[0] {
		if err := s.evalStmt(stmt); err != nil {
			s.popScope()
			s.currentBlock = oldBlock
			return err
		}
	}
	s.popScope()
	s.currentBlock = oldBlock
	return nil
}

func (s *State) evalInclude(inc *parser.Include) error {
	nameVal, err := s.evalExpr(inc.Name)
	if err != nil {
		return err
	}

	name, ok := nameVal.AsString()
	if !ok {
		return NewError(ErrInvalidOperation, "include name must be a string")
	}

	tmpl, err := s.env.GetTemplate(name)
	if err != nil {
		if inc.IgnoreMissing {
			return nil
		}
		return err
	}

	s.depth++
	if s.depth > maxRecursion {
		return NewError(ErrInvalidOperation, "recursion limit exceeded")
	}

	// Create new state sharing the scope
	childState := &State{
		env:        s.env,
		name:       tmpl.compiled.name,
		source:     tmpl.compiled.source,
		autoEscape: s.env.autoEscapeFunc(tmpl.compiled.name),
		scopes:     s.scopes, // Share scopes
		blocks:     s.blocks,
		macros:     s.macros,
		out:        s.out, // Share output
		depth:      s.depth,
	}

	_, err = childState.eval(tmpl.compiled.ast)
	s.depth--
	return err
}

func (s *State) evalImport(imp *parser.Import) error {
	// Evaluate the template path expression
	pathVal, err := s.evalExpr(imp.Expr)
	if err != nil {
		return err
	}

	path, ok := pathVal.AsString()
	if !ok {
		return NewError(ErrInvalidOperation, "import path must be a string")
	}

	// Load and parse the template
	tmpl, err := s.env.GetTemplate(path)
	if err != nil {
		return err
	}

	// Create a module object with all macros from the template
	module := s.createModule(tmpl.compiled.ast)

	// Get the alias name
	var aliasName string
	if varExpr, ok := imp.Name.(*parser.Var); ok {
		aliasName = varExpr.ID
	} else if constExpr, ok := imp.Name.(*parser.Const); ok {
		if name, ok := constExpr.Value.(string); ok {
			aliasName = name
		}
	}
	if aliasName == "" {
		return NewError(ErrInvalidOperation, "import alias must be a name")
	}

	// Set the module in current scope
	s.Set(aliasName, module)
	return nil
}

func (s *State) evalFromImport(frm *parser.FromImport) error {
	// Evaluate the template path expression
	pathVal, err := s.evalExpr(frm.Expr)
	if err != nil {
		return err
	}

	path, ok := pathVal.AsString()
	if !ok {
		return NewError(ErrInvalidOperation, "import path must be a string")
	}

	// Load and parse the template
	tmpl, err := s.env.GetTemplate(path)
	if err != nil {
		return err
	}

	// Create a temporary state to collect macros
	module := s.createModule(tmpl.compiled.ast)
	moduleMap, ok := module.AsMap()
	if !ok {
		moduleMap = make(map[string]value.Value)
	}

	// Import each named item
	for _, name := range frm.Names {
		var importName string
		if varExpr, ok := name.Name.(*parser.Var); ok {
			importName = varExpr.ID
		} else if constExpr, ok := name.Name.(*parser.Const); ok {
			if n, ok := constExpr.Value.(string); ok {
				importName = n
			}
		}
		if importName == "" {
			return NewError(ErrInvalidOperation, "import name must be an identifier")
		}

		// Get the alias (or use the same name)
		aliasName := importName
		if name.Alias != nil {
			if varExpr, ok := name.Alias.(*parser.Var); ok {
				aliasName = varExpr.ID
			} else if constExpr, ok := name.Alias.(*parser.Const); ok {
				if n, ok := constExpr.Value.(string); ok {
					aliasName = n
				}
			}
		}

		// Get the item from the module
		if item, exists := moduleMap[importName]; exists {
			s.Set(aliasName, item)
		} else {
			return NewError(ErrUndefinedVar, fmt.Sprintf("%s not found in %s", importName, path))
		}
	}

	return nil
}

func (s *State) createModule(tmpl *parser.Template) value.Value {
	// Collect all macros from the template
	macros := make(map[string]*parser.Macro)
	for _, stmt := range tmpl.Children {
		if macro, ok := stmt.(*parser.Macro); ok {
			macros[macro.Name] = macro
		}
	}

	// Create a callable map for the module
	module := make(map[string]value.Value)
	for name, macro := range macros {
		// Create a macro callable
		module[name] = s.makeMacroCallable(macro)
	}

	return value.FromMap(module)
}

func (s *State) makeMacroCallable(macro *parser.Macro) value.Value {
	// Store a reference to the macro that can be called later
	// We use a special "callable" value type
	return value.FromCallable(&macroCallable{
		macro: macro,
		state: s,
	})
}

func (s *State) evalFilterBlock(block *parser.FilterBlock) error {
	// Capture output
	oldOut := s.out
	s.out = &strings.Builder{}
	for _, stmt := range block.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return err
		}
	}
	captured := s.out.String()
	s.out = oldOut

	result, err := s.applyFilter(block.Filter, value.FromString(captured))
	if err != nil {
		return err
	}

	s.writeValue(result)
	return nil
}

func (s *State) evalAutoEscape(ae *parser.AutoEscape) error {
	val, err := s.evalExpr(ae.Enabled)
	if err != nil {
		return err
	}

	oldEscape := s.autoEscape

	if b, ok := val.AsBool(); ok {
		if b {
			s.autoEscape = AutoEscapeHTML
		} else {
			s.autoEscape = AutoEscapeNone
		}
	} else if str, ok := val.AsString(); ok {
		switch str {
		case "html":
			s.autoEscape = AutoEscapeHTML
		case "none":
			s.autoEscape = AutoEscapeNone
		default:
			s.autoEscape = AutoEscapeHTML
		}
	}

	for _, stmt := range ae.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.autoEscape = oldEscape
			return err
		}
	}
	s.autoEscape = oldEscape
	return nil
}

func (s *State) writeValue(val value.Value) {
	if val.IsUndefined() {
		return
	}

	str := val.String()
	if s.autoEscape == AutoEscapeHTML && !val.IsSafe() {
		str = EscapeHTML(str)
	}
	s.out.WriteString(str)
}

func (s *State) evalExpr(expr parser.Expr) (value.Value, error) {
	switch e := expr.(type) {
	case *parser.Const:
		return s.evalConst(e), nil

	case *parser.Var:
		return s.Lookup(e.ID), nil

	case *parser.UnaryOp:
		return s.evalUnaryOp(e)

	case *parser.BinOp:
		return s.evalBinOp(e)

	case *parser.IfExpr:
		return s.evalIfExpr(e)

	case *parser.Filter:
		val, err := s.evalExpr(e.Expr)
		if err != nil {
			return value.Undefined(), err
		}
		return s.applyFilterCallArgs(e.Name, val, e.Args)

	case *parser.Test:
		return s.evalTest(e)

	case *parser.GetAttr:
		return s.evalGetAttr(e)

	case *parser.GetItem:
		return s.evalGetItem(e)

	case *parser.Call:
		return s.evalCall(e)

	case *parser.List:
		return s.evalList(e)

	case *parser.Map:
		return s.evalMap(e)

	case *parser.Slice:
		return s.evalSlice(e)

	default:
		return value.Undefined(), fmt.Errorf("unsupported expression type: %T", expr)
	}
}

func (s *State) evalConst(c *parser.Const) value.Value {
	switch v := c.Value.(type) {
	case nil:
		return value.None()
	case bool:
		return value.FromBool(v)
	case int64:
		return value.FromInt(v)
	case float64:
		return value.FromFloat(v)
	case string:
		return value.FromString(v)
	default:
		return value.FromAny(v)
	}
}

func (s *State) evalUnaryOp(op *parser.UnaryOp) (value.Value, error) {
	val, err := s.evalExpr(op.Expr)
	if err != nil {
		return value.Undefined(), err
	}

	switch op.Op {
	case parser.UnaryNot:
		return value.FromBool(!val.IsTrue()), nil
	case parser.UnaryNeg:
		return val.Neg()
	default:
		return value.Undefined(), fmt.Errorf("unknown unary operator")
	}
}

func (s *State) evalBinOp(op *parser.BinOp) (value.Value, error) {
	// Short-circuit evaluation for and/or
	if op.Op == parser.BinOpScAnd {
		left, err := s.evalExpr(op.Left)
		if err != nil {
			return value.Undefined(), err
		}
		if !left.IsTrue() {
			return left, nil
		}
		return s.evalExpr(op.Right)
	}

	if op.Op == parser.BinOpScOr {
		left, err := s.evalExpr(op.Left)
		if err != nil {
			return value.Undefined(), err
		}
		if left.IsTrue() {
			return left, nil
		}
		return s.evalExpr(op.Right)
	}

	left, err := s.evalExpr(op.Left)
	if err != nil {
		return value.Undefined(), err
	}
	right, err := s.evalExpr(op.Right)
	if err != nil {
		return value.Undefined(), err
	}

	switch op.Op {
	case parser.BinOpEq:
		return value.FromBool(left.Equal(right)), nil
	case parser.BinOpNe:
		return value.FromBool(!left.Equal(right)), nil
	case parser.BinOpLt:
		if cmp, ok := left.Compare(right); ok {
			return value.FromBool(cmp < 0), nil
		}
		return value.Undefined(), fmt.Errorf("cannot compare %s and %s", left.Kind(), right.Kind())
	case parser.BinOpLte:
		if cmp, ok := left.Compare(right); ok {
			return value.FromBool(cmp <= 0), nil
		}
		return value.Undefined(), fmt.Errorf("cannot compare %s and %s", left.Kind(), right.Kind())
	case parser.BinOpGt:
		if cmp, ok := left.Compare(right); ok {
			return value.FromBool(cmp > 0), nil
		}
		return value.Undefined(), fmt.Errorf("cannot compare %s and %s", left.Kind(), right.Kind())
	case parser.BinOpGte:
		if cmp, ok := left.Compare(right); ok {
			return value.FromBool(cmp >= 0), nil
		}
		return value.Undefined(), fmt.Errorf("cannot compare %s and %s", left.Kind(), right.Kind())
	case parser.BinOpAdd:
		return left.Add(right)
	case parser.BinOpSub:
		return left.Sub(right)
	case parser.BinOpMul:
		return left.Mul(right)
	case parser.BinOpDiv:
		return left.Div(right)
	case parser.BinOpFloorDiv:
		return left.FloorDiv(right)
	case parser.BinOpRem:
		return left.Rem(right)
	case parser.BinOpPow:
		return left.Pow(right)
	case parser.BinOpConcat:
		return left.Concat(right), nil
	case parser.BinOpIn:
		return value.FromBool(right.Contains(left)), nil
	default:
		return value.Undefined(), fmt.Errorf("unknown binary operator: %v", op.Op)
	}
}

func (s *State) evalIfExpr(ie *parser.IfExpr) (value.Value, error) {
	cond, err := s.evalExpr(ie.TestExpr)
	if err != nil {
		return value.Undefined(), err
	}

	if cond.IsTrue() {
		return s.evalExpr(ie.TrueExpr)
	}

	if ie.FalseExpr != nil {
		return s.evalExpr(ie.FalseExpr)
	}
	return value.Undefined(), nil
}

func (s *State) evalTest(test *parser.Test) (value.Value, error) {
	val, err := s.evalExpr(test.Expr)
	if err != nil {
		return value.Undefined(), err
	}

	var args []value.Value
	for _, arg := range test.Args {
		if arg.Kind == parser.CallArgPos {
			v, err := s.evalExpr(arg.Value)
			if err != nil {
				return value.Undefined(), err
			}
			args = append(args, v)
		}
	}

	testFn, ok := s.env.getTest(test.Name)
	if !ok {
		return value.Undefined(), NewError(ErrUnknownTest, test.Name).WithSpan(test.Span())
	}

	result, err := testFn(s, val, args)
	if err != nil {
		return value.Undefined(), err
	}

	return value.FromBool(result), nil
}

func (s *State) evalGetAttr(ga *parser.GetAttr) (value.Value, error) {
	val, err := s.evalExpr(ga.Expr)
	if err != nil {
		return value.Undefined(), err
	}
	return val.GetAttr(ga.Name), nil
}

func (s *State) evalGetItem(gi *parser.GetItem) (value.Value, error) {
	val, err := s.evalExpr(gi.Expr)
	if err != nil {
		return value.Undefined(), err
	}
	key, err := s.evalExpr(gi.SubscriptExpr)
	if err != nil {
		return value.Undefined(), err
	}
	return val.GetItem(key), nil
}

func (s *State) evalCall(call *parser.Call) (value.Value, error) {
	// Check if it's a function call
	if v, ok := call.Expr.(*parser.Var); ok {
		// Check for super() call
		if v.ID == "super" {
			return s.evalSuper()
		}

		// Check for loop() recursive call
		if v.ID == "loop" && s.loopRecurse != nil {
			if len(call.Args) != 1 {
				return value.Undefined(), NewError(ErrInvalidOperation, "loop() takes exactly 1 argument")
			}
			arg, err := s.evalExpr(call.Args[0].Value)
			if err != nil {
				return value.Undefined(), err
			}
			result, err := s.loopRecurse(arg)
			if err != nil {
				return value.Undefined(), err
			}
			return value.FromSafeString(result), nil
		}

		// Check for macro
		if macro, ok := s.macros[v.ID]; ok {
			return s.callMacroWithArgs(macro, call.Args)
		}

		// Check for function
		if fn, ok := s.env.getFunction(v.ID); ok {
			args, kwargs, err := s.evalCallArgs(call.Args)
			if err != nil {
				return value.Undefined(), err
			}
			return fn(s, args, kwargs)
		}

		// Check if variable is callable
		val := s.Lookup(v.ID)
		if callable, ok := val.AsCallable(); ok {
			args, kwargs, err := s.evalCallArgs(call.Args)
			if err != nil {
				return value.Undefined(), err
			}
			return callable.Call(args, kwargs)
		}
	}

	// Evaluate the expression to get a callable
	expr, err := s.evalExpr(call.Expr)
	if err != nil {
		return value.Undefined(), err
	}

	// Check if it's a callable value
	if callable, ok := expr.AsCallable(); ok {
		args, kwargs, err := s.evalCallArgs(call.Args)
		if err != nil {
			return value.Undefined(), err
		}
		return callable.Call(args, kwargs)
	}

	// Check if it's a method call on a map (like module.macro())
	if getAttr, ok := call.Expr.(*parser.GetAttr); ok {
		obj, err := s.evalExpr(getAttr.Expr)
		if err != nil {
			return value.Undefined(), err
		}
		attr := obj.GetAttr(getAttr.Name)
		if callable, ok := attr.AsCallable(); ok {
			args, kwargs, err := s.evalCallArgs(call.Args)
			if err != nil {
				return value.Undefined(), err
			}
			return callable.Call(args, kwargs)
		}
	}

	return value.Undefined(), NewError(ErrUnknownFunction, "unknown callable").WithSpan(call.Span())
}

func (s *State) evalSuper() (value.Value, error) {
	if s.currentBlock == "" {
		return value.Undefined(), NewError(ErrInvalidOperation, "super() can only be used inside a block")
	}

	bs := s.blocks[s.currentBlock]
	if bs == nil || bs.index+1 >= len(bs.layers) {
		return value.Undefined(), NewError(ErrInvalidOperation, "no parent block exists")
	}

	// Move to the parent block
	bs.index++
	defer func() { bs.index-- }()

	// Capture output
	oldOut := s.out
	s.out = &strings.Builder{}

	s.pushScope()
	for _, stmt := range bs.layers[bs.index] {
		if err := s.evalStmt(stmt); err != nil {
			s.popScope()
			s.out = oldOut
			return value.Undefined(), err
		}
	}
	s.popScope()

	result := s.out.String()
	s.out = oldOut
	return value.FromSafeString(result), nil
}

func (s *State) evalCallArgs(callArgs []parser.CallArg) ([]value.Value, map[string]value.Value, error) {
	var args []value.Value
	kwargs := make(map[string]value.Value)
	for _, arg := range callArgs {
		val, err := s.evalExpr(arg.Value)
		if err != nil {
			return nil, nil, err
		}
		switch arg.Kind {
		case parser.CallArgPos:
			args = append(args, val)
		case parser.CallArgKwarg:
			kwargs[arg.Name] = val
		}
	}
	return args, kwargs, nil
}

func (s *State) callMacroWithArgs(macro *parser.Macro, callArgs []parser.CallArg) (value.Value, error) {
	s.pushScope()
	defer s.popScope()

	// Separate positional and keyword arguments
	var posArgs []value.Value
	kwargs := make(map[string]value.Value)
	for _, arg := range callArgs {
		val, err := s.evalExpr(arg.Value)
		if err != nil {
			return value.Undefined(), err
		}
		if arg.Kind == parser.CallArgKwarg {
			kwargs[arg.Name] = val
		} else {
			posArgs = append(posArgs, val)
		}
	}

	// Bind arguments
	for i, arg := range macro.Args {
		if varArg, ok := arg.(*parser.Var); ok {
			// Check if provided as kwarg
			if val, ok := kwargs[varArg.ID]; ok {
				s.Set(varArg.ID, val)
				continue
			}
			// Check if provided as positional arg
			if i < len(posArgs) {
				s.Set(varArg.ID, posArgs[i])
			} else if i-len(macro.Args)+len(macro.Defaults) >= 0 {
				// Use default value
				defaultIdx := i - len(macro.Args) + len(macro.Defaults)
				if defaultIdx >= 0 && defaultIdx < len(macro.Defaults) {
					val, err := s.evalExpr(macro.Defaults[defaultIdx])
					if err != nil {
						return value.Undefined(), err
					}
					s.Set(varArg.ID, val)
				} else {
					s.Set(varArg.ID, value.Undefined())
				}
			} else {
				s.Set(varArg.ID, value.Undefined())
			}
		}
	}

	// Capture output
	oldOut := s.out
	s.out = &strings.Builder{}
	for _, stmt := range macro.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return value.Undefined(), err
		}
	}
	result := s.out.String()
	s.out = oldOut

	return value.FromSafeString(result), nil
}

func (s *State) evalList(list *parser.List) (value.Value, error) {
	items := make([]value.Value, len(list.Items))
	for i, item := range list.Items {
		var err error
		items[i], err = s.evalExpr(item)
		if err != nil {
			return value.Undefined(), err
		}
	}
	return value.FromSlice(items), nil
}

func (s *State) evalMap(m *parser.Map) (value.Value, error) {
	result := make(map[string]value.Value)
	for i := range m.Keys {
		key, err := s.evalExpr(m.Keys[i])
		if err != nil {
			return value.Undefined(), err
		}
		val, err := s.evalExpr(m.Values[i])
		if err != nil {
			return value.Undefined(), err
		}
		keyStr, ok := key.AsString()
		if !ok {
			keyStr = key.String()
		}
		result[keyStr] = val
	}
	return value.FromMap(result), nil
}

func (s *State) evalSlice(sl *parser.Slice) (value.Value, error) {
	val, err := s.evalExpr(sl.Expr)
	if err != nil {
		return value.Undefined(), err
	}

	var start, stop *int64
	var step int64 = 1

	if sl.Start != nil {
		v, err := s.evalExpr(sl.Start)
		if err != nil {
			return value.Undefined(), err
		}
		if i, ok := v.AsInt(); ok {
			start = &i
		}
	}

	if sl.Stop != nil {
		v, err := s.evalExpr(sl.Stop)
		if err != nil {
			return value.Undefined(), err
		}
		if i, ok := v.AsInt(); ok {
			stop = &i
		}
	}

	if sl.Step != nil {
		v, err := s.evalExpr(sl.Step)
		if err != nil {
			return value.Undefined(), err
		}
		if i, ok := v.AsInt(); ok {
			step = i
		}
	}

	return s.sliceValue(val, start, stop, step)
}

func (s *State) sliceValue(val value.Value, start, stop *int64, step int64) (value.Value, error) {
	if step == 0 {
		return value.Undefined(), fmt.Errorf("slice step cannot be zero")
	}

	switch {
	case val.Kind() == value.KindSeq:
		items, _ := val.AsSlice()
		return value.FromSlice(sliceSlice(items, start, stop, step)), nil
	case val.Kind() == value.KindString:
		str, _ := val.AsString()
		runes := []rune(str)
		result := sliceRunes(runes, start, stop, step)
		if val.IsSafe() {
			return value.FromSafeString(string(result)), nil
		}
		return value.FromString(string(result)), nil
	default:
		return value.Undefined(), fmt.Errorf("cannot slice %s", val.Kind())
	}
}

func sliceSlice(items []value.Value, start, stop *int64, step int64) []value.Value {
	length := int64(len(items))
	s, e := resolveSliceIndices(length, start, stop, step)

	var result []value.Value
	if step > 0 {
		for i := s; i < e; i += step {
			result = append(result, items[i])
		}
	} else {
		for i := s; i > e; i += step {
			result = append(result, items[i])
		}
	}
	return result
}

func sliceRunes(runes []rune, start, stop *int64, step int64) []rune {
	length := int64(len(runes))
	s, e := resolveSliceIndices(length, start, stop, step)

	var result []rune
	if step > 0 {
		for i := s; i < e; i += step {
			result = append(result, runes[i])
		}
	} else {
		for i := s; i > e; i += step {
			result = append(result, runes[i])
		}
	}
	return result
}

func resolveSliceIndices(length int64, start, stop *int64, step int64) (int64, int64) {
	var s, e int64

	if step > 0 {
		if start == nil {
			s = 0
		} else {
			s = normalizeIndex(*start, length)
		}
		if stop == nil {
			e = length
		} else {
			e = normalizeIndex(*stop, length)
		}
		if s < 0 {
			s = 0
		}
		if e > length {
			e = length
		}
	} else {
		if start == nil {
			s = length - 1
		} else {
			s = normalizeIndex(*start, length)
		}
		if stop == nil {
			e = -1
		} else {
			e = normalizeIndex(*stop, length)
		}
		if s >= length {
			s = length - 1
		}
		if e < -1 {
			e = -1
		}
	}

	return s, e
}

func normalizeIndex(idx, length int64) int64 {
	if idx < 0 {
		idx = length + idx
	}
	return idx
}

func (s *State) applyFilter(filterExpr parser.Expr, val value.Value) (value.Value, error) {
	switch f := filterExpr.(type) {
	case *parser.Filter:
		return s.applyFilterCallArgs(f.Name, val, f.Args)
	case *parser.Var:
		return s.applyFilterCallArgs(f.ID, val, nil)
	default:
		return value.Undefined(), fmt.Errorf("invalid filter expression")
	}
}

func (s *State) applyFilterCallArgs(name string, val value.Value, callArgs []parser.CallArg) (value.Value, error) {
	filterFn, ok := s.env.getFilter(name)
	if !ok {
		return value.Undefined(), NewError(ErrUnknownFilter, name)
	}

	var args []value.Value
	kwargs := make(map[string]value.Value)
	for _, arg := range callArgs {
		v, err := s.evalExpr(arg.Value)
		if err != nil {
			return value.Undefined(), err
		}
		if arg.Kind == parser.CallArgKwarg {
			kwargs[arg.Name] = v
		} else {
			args = append(args, v)
		}
	}

	return filterFn(s, val, args, kwargs)
}
