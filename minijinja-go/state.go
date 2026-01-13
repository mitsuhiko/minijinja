package minijinja

import (
	"fmt"
	"sort"
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
	macros       map[string]*macroDefinition
	out          *strings.Builder
	depth        int
	currentBlock string                            // name of block currently being rendered
	loopRecurse       func(value.Value) (string, error) // for recursive loops
	undefinedBehavior UndefinedBehavior
}

// blockStack manages the inheritance chain for a single block
type blockStack struct {
	layers [][]parser.Stmt // stack of block implementations (child first)
	index  int             // current index in stack
}

type macroDefinition struct {
	macro  *parser.Macro
	state  *State
	scopes []map[string]value.Value
}

// macroCallable wraps a macro for callable invocation
type macroCallable struct {
	macro     *parser.Macro
	state     *State
	caller    value.Value // caller value if this is a call block macro
	hasCaller bool
	scopes    []map[string]value.Value
}

func newMacroDefinition(state *State, macro *parser.Macro) *macroDefinition {
	return &macroDefinition{
		macro:  macro,
		state:  state,
		scopes: cloneScopes(state.scopes),
	}
}

func newMacroCallable(state *State, macro *parser.Macro, caller value.Value) *macroCallable {
	return newMacroCallableFromDefinition(&macroDefinition{
		macro:  macro,
		state:  state,
		scopes: cloneScopes(state.scopes),
	}, caller)
}

func newMacroCallableFromDefinition(def *macroDefinition, caller value.Value) *macroCallable {
	return &macroCallable{
		macro:     def.macro,
		state:     def.state,
		caller:    caller,
		hasCaller: macroUsesCaller(def.macro),
		scopes:    cloneScopes(def.scopes),
	}
}

func (m *macroCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	oldScopes := m.state.scopes
	m.state.scopes = cloneScopes(m.scopes)
	defer func() {
		m.state.scopes = oldScopes
	}()
	return m.state.callMacroWithValues(m.macro, args, kwargs, m.caller)
}

// GetAttr returns macro properties
func (m *macroCallable) GetAttr(name string) value.Value {
	switch name {
	case "name":
		return value.FromString(m.macro.Name)
	case "arguments":
		argNames := make([]value.Value, len(m.macro.Args))
		for i, arg := range m.macro.Args {
			if v, ok := arg.(*parser.Var); ok {
				argNames[i] = value.FromString(v.ID)
			}
		}
		return value.FromSlice(argNames)
	case "caller":
		return value.FromBool(m.hasCaller)
	}
	return value.Undefined()
}

func (m *macroCallable) String() string {
	return fmt.Sprintf("<macro %s>", m.macro.Name)
}

// functionCallable wraps a global function for callable usage in templates.
type functionCallable struct {
	state *State
	fn    FunctionFunc
	name  string
}

func (f *functionCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	return f.fn(f.state, args, kwargs)
}

func (f *functionCallable) String() string {
	if f.name == "" {
		return "<function>"
	}
	return fmt.Sprintf("<function %s>", f.name)
}

func macroUsesCaller(macro *parser.Macro) bool {
	for _, stmt := range macro.Body {
		if stmtUsesCaller(stmt) {
			return true
		}
	}
	return false
}

func stmtUsesCaller(stmt parser.Stmt) bool {
	switch st := stmt.(type) {
	case *parser.EmitExpr:
		return exprUsesCaller(st.Expr)
	case *parser.ForLoop:
		if exprUsesCaller(st.Target) || exprUsesCaller(st.Iter) || exprUsesCaller(st.FilterExpr) {
			return true
		}
		for _, bodyStmt := range st.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
		for _, bodyStmt := range st.ElseBody {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.IfCond:
		if exprUsesCaller(st.Expr) {
			return true
		}
		for _, bodyStmt := range st.TrueBody {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
		for _, bodyStmt := range st.FalseBody {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.WithBlock:
		for _, assignment := range st.Assignments {
			if exprUsesCaller(assignment.Target) || exprUsesCaller(assignment.Value) {
				return true
			}
		}
		for _, bodyStmt := range st.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.Set:
		return exprUsesCaller(st.Target) || exprUsesCaller(st.Expr)
	case *parser.SetBlock:
		if exprUsesCaller(st.Target) || exprUsesCaller(st.Filter) {
			return true
		}
		for _, bodyStmt := range st.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.Block:
		for _, bodyStmt := range st.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.Extends:
		return exprUsesCaller(st.Name)
	case *parser.Import:
		return exprUsesCaller(st.Expr) || exprUsesCaller(st.Name)
	case *parser.FromImport:
		if exprUsesCaller(st.Expr) {
			return true
		}
		for _, name := range st.Names {
			if exprUsesCaller(name.Name) || exprUsesCaller(name.Alias) {
				return true
			}
		}
		return false
	case *parser.Include:
		return exprUsesCaller(st.Name)
	case *parser.Macro:
		for _, bodyStmt := range st.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.FilterBlock:
		if exprUsesCaller(st.Filter) {
			return true
		}
		for _, bodyStmt := range st.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.AutoEscape:
		if exprUsesCaller(st.Enabled) {
			return true
		}
		for _, bodyStmt := range st.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	case *parser.Do:
		return exprUsesCaller(st.Call)
	case *parser.CallBlock:
		if exprUsesCaller(st.Call) {
			return true
		}
		for _, bodyStmt := range st.MacroDecl.Body {
			if stmtUsesCaller(bodyStmt) {
				return true
			}
		}
	}
	return false
}

func exprUsesCaller(expr parser.Expr) bool {
	switch e := expr.(type) {
	case *parser.Var:
		return e.ID == "caller"
	case *parser.UnaryOp:
		return exprUsesCaller(e.Expr)
	case *parser.BinOp:
		return exprUsesCaller(e.Left) || exprUsesCaller(e.Right)
	case *parser.IfExpr:
		return exprUsesCaller(e.TestExpr) || exprUsesCaller(e.TrueExpr) || exprUsesCaller(e.FalseExpr)
	case *parser.Filter:
		if exprUsesCaller(e.Expr) {
			return true
		}
		for _, arg := range e.Args {
			if exprUsesCaller(arg.Value) {
				return true
			}
		}
	case *parser.Test:
		if exprUsesCaller(e.Expr) {
			return true
		}
		for _, arg := range e.Args {
			if exprUsesCaller(arg.Value) {
				return true
			}
		}
	case *parser.GetAttr:
		return exprUsesCaller(e.Expr)
	case *parser.GetItem:
		return exprUsesCaller(e.Expr) || exprUsesCaller(e.SubscriptExpr)
	case *parser.Call:
		if exprUsesCaller(e.Expr) {
			return true
		}
		for _, arg := range e.Args {
			if exprUsesCaller(arg.Value) {
				return true
			}
		}
	case *parser.List:
		for _, item := range e.Items {
			if exprUsesCaller(item) {
				return true
			}
		}
	case *parser.Map:
		for _, item := range e.Keys {
			if exprUsesCaller(item) {
				return true
			}
		}
		for _, item := range e.Values {
			if exprUsesCaller(item) {
				return true
			}
		}
	case *parser.Slice:
		return exprUsesCaller(e.Expr) || exprUsesCaller(e.Start) || exprUsesCaller(e.Stop) || exprUsesCaller(e.Step)
	}
	return false
}

func (s *State) callMacroWithValues(macro *parser.Macro, args []value.Value, kwargs map[string]value.Value, caller value.Value) (value.Value, error) {
	s.depth++
	if s.depth > maxRecursion {
		return value.Undefined(), NewError(ErrInvalidOperation, "recursion limit exceeded")
	}
	defer func() { s.depth-- }()

	s.pushScope()
	defer s.popScope()

	if len(args) > len(macro.Args) {
		return value.Undefined(), NewError(ErrTooManyArguments, "too many arguments")
	}

	remainingKwargs := make(map[string]value.Value, len(kwargs))
	for k, v := range kwargs {
		remainingKwargs[k] = v
	}

	// Bind arguments
	for i, arg := range macro.Args {
		if varArg, ok := arg.(*parser.Var); ok {
			// Check if provided as kwarg
			if val, ok := remainingKwargs[varArg.ID]; ok {
				if i < len(args) {
					return value.Undefined(), NewError(ErrTooManyArguments, "multiple values for argument")
				}
				s.Set(varArg.ID, val)
				delete(remainingKwargs, varArg.ID)
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

	if len(remainingKwargs) > 0 {
		return value.Undefined(), NewError(ErrTooManyArguments, "too many keyword arguments")
	}

	// Set caller if provided
	if !caller.IsUndefined() {
		s.Set("caller", caller)
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

// loopObject is the loop variable object that supports cycle() and previtem/nextitem
type loopObject struct {
	index     int           // 0-based index
	length    int           // total length
	depth     int           // nesting depth (0-based)
	items     []value.Value // all items for previtem/nextitem
	changed   *value.Value  // last value for changed()
	prevItem  value.Value
	oneShot   *value.OneShotIterator
	recurseFn func(value.Value) (string, error)
}

func (l *loopObject) GetAttr(name string) value.Value {
	switch name {
	case "index":
		return value.FromInt(int64(l.index + 1))
	case "index0":
		return value.FromInt(int64(l.index))
	case "revindex":
		return value.FromInt(int64(l.length - l.index))
	case "revindex0":
		return value.FromInt(int64(l.length - l.index - 1))
	case "first":
		return value.FromBool(l.index == 0)
	case "last":
		return value.FromBool(l.index == l.length-1)
	case "length":
		return value.FromInt(int64(l.length))
	case "depth":
		return value.FromInt(int64(l.depth + 1))
	case "depth0":
		return value.FromInt(int64(l.depth))
	case "previtem":
		if l.oneShot != nil {
			return l.prevItem
		}
		if l.index > 0 {
			return l.items[l.index-1]
		}
		return value.Undefined()
	case "nextitem":
		if l.oneShot != nil {
			return l.oneShot.Peek()
		}
		if l.index < l.length-1 {
			return l.items[l.index+1]
		}
		return value.Undefined()
	case "cycle":
		return value.FromCallable(&loopCycleCallable{loop: l})
	case "changed":
		return value.FromCallable(&loopChangedCallable{loop: l})
	}
	return value.Undefined()
}

func (l *loopObject) String() string {
	return fmt.Sprintf("<loop %d/%d>", l.index, l.length)
}

func (l *loopObject) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if l.recurseFn == nil {
		return value.Undefined(), NewError(ErrInvalidOperation, "loop recursion cannot be called this way")
	}
	if len(args) != 1 {
		return value.Undefined(), NewError(ErrInvalidOperation, "loop() takes exactly 1 argument")
	}
	result, err := l.recurseFn(args[0])
	if err != nil {
		return value.Undefined(), err
	}
	return value.FromSafeString(result), nil
}

func (l *loopObject) Map() map[string]value.Value {
	return map[string]value.Value{
		"index":     value.FromInt(int64(l.index + 1)),
		"index0":    value.FromInt(int64(l.index)),
		"revindex":  value.FromInt(int64(l.length - l.index)),
		"revindex0": value.FromInt(int64(l.length - l.index - 1)),
		"first":     value.FromBool(l.index == 0),
		"last":      value.FromBool(l.index == l.length-1),
		"length":    value.FromInt(int64(l.length)),
		"depth":     value.FromInt(int64(l.depth + 1)),
		"depth0":    value.FromInt(int64(l.depth)),
		"previtem":  l.GetAttr("previtem"),
		"nextitem":  l.GetAttr("nextitem"),
	}
}

// loopCycleCallable implements loop.cycle()
type loopCycleCallable struct {
	loop *loopObject
}

func (c *loopCycleCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if len(args) == 0 {
		return value.Undefined(), nil
	}
	idx := c.loop.index % len(args)
	return args[idx], nil
}

// loopChangedCallable implements loop.changed()
type loopChangedCallable struct {
	loop *loopObject
}

func (c *loopChangedCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	// Create a comparable representation of args
	newVal := value.FromSlice(args)
	if c.loop.changed == nil {
		c.loop.changed = &newVal
		return value.FromBool(true), nil
	}
	changed := !newVal.Equal(*c.loop.changed)
	if changed {
		c.loop.changed = &newVal
	}
	return value.FromBool(changed), nil
}

// selfObject provides access to blocks via self.blockname()
type selfObject struct {
	state *State
}

func (so *selfObject) GetAttr(name string) value.Value {
	// Return a callable that renders the block
	return value.FromCallable(&blockCallable{
		state:     so.state,
		blockName: name,
	})
}

// blockCallable renders a block when called
type blockCallable struct {
	state     *State
	blockName string
}

func (bc *blockCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	bs := bc.state.blocks[bc.blockName]
	if bs == nil || len(bs.layers) == 0 {
		return value.Undefined(), NewError(ErrInvalidOperation, fmt.Sprintf("block '%s' not found", bc.blockName))
	}

	// Capture output
	oldOut := bc.state.out
	bc.state.out = &strings.Builder{}

	oldBlock := bc.state.currentBlock
	bc.state.currentBlock = bc.blockName

	bc.state.pushScope()
	for _, stmt := range bs.layers[0] {
		if err := bc.state.evalStmt(stmt); err != nil {
			bc.state.popScope()
			bc.state.currentBlock = oldBlock
			bc.state.out = oldOut
			return value.Undefined(), err
		}
	}
	bc.state.popScope()

	result := bc.state.out.String()
	bc.state.out = oldOut
	bc.state.currentBlock = oldBlock

	return value.FromSafeString(result), nil
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
		blocks:            make(map[string]*blockStack),
		macros:            make(map[string]*macroDefinition),
		out:               &strings.Builder{},
		undefinedBehavior: env.undefinedBehavior,
	}
}

// Lookup looks up a variable in the current scope chain.
func (s *State) Lookup(name string) value.Value {
	// Special handling for "self"
	if name == "self" {
		return value.FromObject(&selfObject{state: s})
	}

	// Search scopes from inner to outer
	for i := len(s.scopes) - 1; i >= 0; i-- {
		if v, ok := s.scopes[i][name]; ok {
			return v
		}
	}

	// Check macros
	if macro, ok := s.macros[name]; ok {
		return value.FromCallable(newMacroCallableFromDefinition(macro, value.Undefined()))
	}

	// Check globals
	if v, ok := s.env.getGlobal(name); ok {
		return v
	}

	// Check functions as callables
	if fn, ok := s.env.getFunction(name); ok {
		return value.FromCallable(&functionCallable{state: s, fn: fn, name: name})
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
			if extendsStmt != nil {
				return "", NewError(ErrInvalidOperation, "tried to extend a second time in a template")
			}
			extendsStmt = ext
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
				s.macros[macro.Name] = newMacroDefinition(s, macro)
			}
		}

		// Evaluate non-output statements (like set/import) before rendering parent
		for _, stmt := range tmpl.Children {
			switch st := stmt.(type) {
			case *parser.Set:
				if err := s.evalSet(st); err != nil {
					return "", err
				}
			case *parser.SetBlock:
				if err := s.evalSetBlock(st); err != nil {
					return "", err
				}
			case *parser.Import:
				if err := s.evalImport(st); err != nil {
					return "", err
				}
			case *parser.FromImport:
				if err := s.evalFromImport(st); err != nil {
					return "", err
				}
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
		s.macros[st.Name] = newMacroDefinition(s, st)
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

	case *parser.CallBlock:
		return s.evalCallBlock(st)

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

	if oneShot, ok := iter.Raw().(*value.OneShotIterator); ok {
		if loop.Recursive || loop.FilterExpr != nil {
			items := oneShot.Drain()
			return s.evalForLoopItems(loop, items)
		}
		return s.evalForLoopOneShot(loop, oneShot)
	}

	items := iter.Iter()
	if items == nil {
		if iter.IsUndefined() || iter.IsNone() {
			if iter.IsUndefined() && s.undefinedBehavior == UndefinedStrict {
				return NewError(ErrUndefinedVar, "undefined value")
			}
			items = []value.Value{}
		} else {
			return NewError(ErrInvalidOperation, fmt.Sprintf("%s is not iterable", iter.Kind()))
		}
	}

	return s.evalForLoopItems(loop, items)
}

func (s *State) evalForLoopOneShot(loop *parser.ForLoop, iter *value.OneShotIterator) error {
	if iter.Remaining() == 0 {
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

	index := 0
	prevItem := value.Undefined()
	for {
		item, ok := iter.Next()
		if !ok {
			break
		}
		if err := s.unpackLoopTarget(loop.Target, item); err != nil {
			return err
		}

		loopObj := &loopObject{
			index:     index,
			length:    index + iter.Remaining() + 1,
			depth:     s.depth - 1,
			items:     nil,
			prevItem:  prevItem,
			oneShot:   iter,
			recurseFn: s.loopRecurse,
		}
		s.Set("loop", value.FromObject(loopObj))

		for _, stmt := range loop.Body {
			err := s.evalStmt(stmt)
			if err == errContinue {
				break
			}
			if err == errBreak {
				iter.DiscardBuffered()
				return nil
			}
			if err != nil {
				return err
			}
		}

		prevItem = item
		index++
	}

	return nil
}

func (s *State) evalForLoopItems(loop *parser.ForLoop, items []value.Value) error {
	// Apply filter if present
	if loop.FilterExpr != nil {
		filtered := make([]value.Value, 0, len(items))
		s.pushScope()
		for _, item := range items {
			if err := s.unpackLoopTarget(loop.Target, item); err != nil {
				s.popScope()
				return err
			}
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
			s.depth++
			if s.depth > maxRecursion {
				return "", NewError(ErrInvalidOperation, "recursion limit exceeded")
			}
			defer func() { s.depth-- }()

			s.pushScope()
			defer s.popScope()

			nestedItems := iterValue.Iter()
			if nestedItems == nil {
				if iterValue.IsUndefined() || iterValue.IsNone() {
					return "", nil
				}
				return "", NewError(ErrInvalidOperation, "cannot recurse because of non-iterable value")
			}

			oldOut := s.out
			s.out = &strings.Builder{}

			for i := range nestedItems {
				if err := s.unpackLoopTarget(loop.Target, nestedItems[i]); err != nil {
					s.out = oldOut
					return "", err
				}

				loopObj := &loopObject{
					index:     i,
					length:    len(nestedItems),
					depth:     s.depth - 1,
					items:     nestedItems,
					recurseFn: s.loopRecurse,
				}
				s.Set("loop", value.FromObject(loopObj))

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

	for i := range items {
		if err := s.unpackLoopTarget(loop.Target, items[i]); err != nil {
			return err
		}

		// Set loop variable as an object
		loopObj := &loopObject{
			index:     i,
			length:    len(items),
			depth:     s.depth - 1,
			items:     items,
			recurseFn: s.loopRecurse,
		}
		s.Set("loop", value.FromObject(loopObj))

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
		items, ok := val.AsSlice()
		if !ok {
			items = val.Iter()
		}
		if items != nil {
			for i, item := range t.Items {
				if i < len(items) {
					s.unpackTarget(item, items[i])
				} else {
					s.unpackTarget(item, value.Undefined())
				}
			}
		} else {
			for _, item := range t.Items {
				s.unpackTarget(item, value.Undefined())
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

func (s *State) unpackLoopTarget(target parser.Expr, val value.Value) error {
	if list, ok := target.(*parser.List); ok {
		items, ok := val.AsSlice()
		if !ok {
			items = val.Iter()
		}
		if items == nil {
			return NewError(ErrInvalidOperation, "cannot unpack non-iterable")
		}
		if len(items) != len(list.Items) {
			return NewError(ErrInvalidOperation, "wrong number of values to unpack")
		}
	}
	s.unpackTarget(target, val)
	return nil
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
				s.macros[macro.Name] = newMacroDefinition(s, macro)
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
	if bs == nil {
		bs = &blockStack{
			layers: [][]parser.Stmt{block.Body},
			index:  0,
		}
		s.blocks[block.Name] = bs
	}
	if len(bs.layers) == 0 {
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

	if name, ok := nameVal.AsString(); ok {
		if err := s.includeTemplate(name); err != nil {
			if inc.IgnoreMissing && isTemplateNotFound(err) {
				return nil
			}
			return err
		}
		return nil
	}

	if items := nameVal.Iter(); items != nil {
		var lastErr error
		for _, item := range items {
			name, ok := item.AsString()
			if !ok {
				name = item.String()
			}
			if name == "" {
				continue
			}
			if err := s.includeTemplate(name); err != nil {
				lastErr = err
				continue
			}
			return nil
		}

		if inc.IgnoreMissing {
			return nil
		}
		if lastErr != nil {
			return lastErr
		}
		return NewError(ErrTemplateNotFound, "tried to include one of multiple templates, none of which existed")
	}

	return NewError(ErrInvalidOperation, "include name must be a string")
}

func (s *State) includeTemplate(name string) error {
	tmpl, err := s.env.GetTemplate(name)
	if err != nil {
		return err
	}

	s.depth++
	if s.depth > maxRecursion {
		return NewError(ErrInvalidOperation, "recursion limit exceeded")
	}

	// Create new state with isolated scope
	childState := &State{
		env:        s.env,
		name:       tmpl.compiled.name,
		source:     tmpl.compiled.source,
		autoEscape: s.env.autoEscapeFunc(tmpl.compiled.name),
		scopes:     cloneScopes(s.scopes),
		blocks:            s.blocks,
		macros:            s.macros,
		out:               s.out,
		depth:             s.depth,
		undefinedBehavior: s.undefinedBehavior,
	}
	if len(childState.scopes) > 0 {
		childState.scopes[len(childState.scopes)-1] = cloneScopeMap(childState.scopes[len(childState.scopes)-1])
	}

	_, err = childState.eval(tmpl.compiled.ast)
	s.depth--
	return err
}

func isTemplateNotFound(err error) bool {
	if err == nil {
		return false
	}
	if templErr, ok := err.(*Error); ok {
		return templErr.Kind == ErrTemplateNotFound
	}
	return false
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
	module, err := s.createModule(tmpl.compiled)
	if err != nil {
		return err
	}

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
	module, err := s.createModule(tmpl.compiled)
	if err != nil {
		return err
	}
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
			s.Set(aliasName, value.Undefined())
		}
	}

	return nil
}

func (s *State) createModule(tmpl *compiledTemplate) (value.Value, error) {
	moduleState := &State{
		env:        s.env,
		name:       tmpl.name,
		source:     tmpl.source,
		autoEscape: s.env.autoEscapeFunc(tmpl.name),
		scopes:     cloneScopes(s.scopes),
		blocks:            make(map[string]*blockStack),
		macros:            make(map[string]*macroDefinition),
		out:               &strings.Builder{},
		depth:             s.depth,
		undefinedBehavior: s.undefinedBehavior,
	}
	moduleState.pushScope()

	captured, err := moduleState.eval(tmpl.ast)
	if err != nil {
		return value.Undefined(), err
	}

	moduleValues := make(map[string]value.Value)
	for k, v := range moduleState.scopes[len(moduleState.scopes)-1] {
		moduleValues[k] = v
	}
	for name, macro := range moduleState.macros {
		moduleValues[name] = value.FromCallable(newMacroCallableFromDefinition(macro, value.Undefined()))
	}

	return value.FromObject(&moduleObject{
		values:   moduleValues,
		captured: captured,
	}), nil
}

func cloneScopes(scopes []map[string]value.Value) []map[string]value.Value {
	newScopes := make([]map[string]value.Value, len(scopes))
	copy(newScopes, scopes)
	return newScopes
}

func cloneScopeMap(scope map[string]value.Value) map[string]value.Value {
	newScope := make(map[string]value.Value, len(scope))
	for k, v := range scope {
		newScope[k] = v
	}
	return newScope
}

type moduleObject struct {
	values   map[string]value.Value
	captured string
}

func (m *moduleObject) GetAttr(name string) value.Value {
	if v, ok := m.values[name]; ok {
		return v
	}
	return value.Undefined()
}

func (m *moduleObject) Map() map[string]value.Value {
	return m.values
}

func (m *moduleObject) String() string {
	return m.captured
}

func (s *State) makeMacroCallable(macro *parser.Macro) value.Value {
	return value.FromCallable(newMacroCallable(s, macro, value.Undefined()))
}

func (s *State) evalFilterBlock(block *parser.FilterBlock) error {
	// Capture output
	oldOut := s.out
	oldEscape := s.autoEscape
	s.out = &strings.Builder{}
	for _, stmt := range block.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return err
		}
	}
	captured := s.out.String()
	s.out = oldOut

	capturedVal := value.FromString(captured)
	if oldEscape != AutoEscapeNone {
		capturedVal = value.FromSafeString(captured)
	}

	result, err := s.applyFilter(block.Filter, capturedVal)
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

func (s *State) evalCallBlock(cb *parser.CallBlock) error {
	// Evaluate the call expression to get the macro
	callExpr := cb.Call

	// Get the macro being called
	var macroDef *macroDefinition
	var macroName string

	if v, ok := callExpr.Expr.(*parser.Var); ok {
		macroName = v.ID
		if m, ok := s.macros[macroName]; ok {
			macroDef = m
		}
	}

	if macroDef == nil {
		// Try to evaluate as a callable
		expr, err := s.evalExpr(callExpr.Expr)
		if err != nil {
			return err
		}
		if mc, ok := expr.AsObject(); ok {
			if macroC, ok := mc.(*macroCallable); ok {
				macroDef = &macroDefinition{
					macro:  macroC.macro,
					state:  macroC.state,
					scopes: cloneScopes(macroC.scopes),
				}
			}
		}
	}

	if macroDef == nil {
		return NewError(ErrInvalidOperation, "call block requires a macro")
	}
	if !macroUsesCaller(macroDef.macro) {
		return NewError(ErrInvalidOperation, "caller is not allowed")
	}

	// Create a caller callable that renders the call block body
	// MacroDecl holds the caller's body and arguments
	callerCallable := &callerCallable{
		state: s,
		body:  cb.MacroDecl.Body,
		args:  cb.MacroDecl.Args,
	}

	// Evaluate call arguments
	args, kwargs, err := s.evalCallArgs(callExpr.Args)
	if err != nil {
		return err
	}

	// Call the macro with the caller
	result, err := newMacroCallableFromDefinition(macroDef, value.FromCallable(callerCallable)).Call(args, kwargs)
	if err != nil {
		return err
	}

	s.writeValue(result)
	return nil
}

// callerCallable represents the caller() function inside a macro invoked via call block
type callerCallable struct {
	state *State
	body  []parser.Stmt
	args  []parser.Expr // macro args from the call block declaration
}

func (c *callerCallable) GetAttr(name string) value.Value {
	switch name {
	case "name":
		return value.FromString("caller")
	case "arguments":
		argNames := make([]value.Value, len(c.args))
		for i, arg := range c.args {
			if v, ok := arg.(*parser.Var); ok {
				argNames[i] = value.FromString(v.ID)
			}
		}
		return value.FromSlice(argNames)
	case "caller":
		return value.False()
	}
	return value.Undefined()
}

func (c *callerCallable) String() string {
	return "<macro caller>"
}

func (c *callerCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	c.state.pushScope()
	defer c.state.popScope()

	// Bind arguments to the caller's parameters
	for i, argExpr := range c.args {
		if v, ok := argExpr.(*parser.Var); ok {
			if i < len(args) {
				c.state.Set(v.ID, args[i])
			} else {
				c.state.Set(v.ID, value.Undefined())
			}
		}
	}

	// Capture output
	oldOut := c.state.out
	c.state.out = &strings.Builder{}

	for _, stmt := range c.body {
		if err := c.state.evalStmt(stmt); err != nil {
			c.state.out = oldOut
			return value.Undefined(), err
		}
	}

	result := c.state.out.String()
	c.state.out = oldOut

	return value.FromSafeString(result), nil
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
		val := s.Lookup(e.ID)
		if val.IsUndefined() && s.undefinedBehavior == UndefinedStrict {
			return value.Undefined(), NewError(ErrUndefinedVar, "undefined value").WithSpan(e.Span())
		}
		return val, nil

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
	case *parser.BigInt:
		return value.FromBigInt(v.Int)
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
	if val.IsUndefined() {
		return value.Undefined(), NewError(ErrUndefinedVar, "undefined value").WithSpan(ga.Span())
	}
	return val.GetAttr(ga.Name), nil
}

func (s *State) evalGetItem(gi *parser.GetItem) (value.Value, error) {
	val, err := s.evalExpr(gi.Expr)
	if err != nil {
		return value.Undefined(), err
	}
	if val.IsUndefined() {
		return value.Undefined(), NewError(ErrUndefinedVar, "undefined value").WithSpan(gi.Span())
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
			args, kwargs, err := s.evalCallArgs(call.Args)
			if err != nil {
				return value.Undefined(), err
			}
			return newMacroCallableFromDefinition(macro, value.Undefined()).Call(args, kwargs)
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
		case parser.CallArgPosSplat:
			items := val.Iter()
			if items == nil {
				return nil, nil, NewError(ErrInvalidOperation, "cannot unpack non-iterable")
			}
			args = append(args, items...)
		case parser.CallArgKwargSplat:
			m, ok := val.AsMap()
			if !ok {
				return nil, nil, NewError(ErrInvalidOperation, "cannot unpack non-map")
			}
			for k, v := range m {
				kwargs[k] = v
			}
		}
	}
	return args, kwargs, nil
}

func (s *State) callMacroWithArgs(macro *parser.Macro, callArgs []parser.CallArg) (value.Value, error) {
	args, kwargs, err := s.evalCallArgs(callArgs)
	if err != nil {
		return value.Undefined(), err
	}
	return s.callMacroWithValues(macro, args, kwargs, value.Undefined())
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

	args, kwargs, err := s.evalCallArgs(callArgs)
	if err != nil {
		return value.Undefined(), err
	}

	return filterFn(s, val, args, kwargs)
}

// debugObject represents the debug() function result with proper repr
type debugObject struct {
	repr string
}

func (d *debugObject) GetAttr(name string) value.Value {
	return value.Undefined()
}

func (d *debugObject) String() string {
	return d.repr
}

// Stringer interface for namespace output
type namespaceStringer interface {
	String() string
}

// Helper to get sorted keys from scope
func sortedScopeKeys(scope map[string]value.Value) []string {
	keys := make([]string, 0, len(scope))
	for k := range scope {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
