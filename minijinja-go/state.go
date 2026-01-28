package minijinja

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"
	"sync"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/parser"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// State holds the evaluation state during template rendering.
//
// The State is passed to filters, tests, and functions to provide access
// to the rendering context. It provides methods for looking up variables,
// accessing the environment, and retrieving the context.Context.
//
// State is also returned by Template.EvalToState(), allowing you to
// render individual blocks, call macros, and inspect template exports
// after evaluation.
type State struct {
	ctx               context.Context
	env               *Environment
	name              string
	source            string
	autoEscape        AutoEscape
	scopes            []map[string]value.Value
	blocks            map[string]*blockStack
	macros            map[string]*macroDefinition
	out               outputWriter
	depth             int
	currentBlock      string                            // name of block currently being rendered
	loopRecurse       func(value.Value) (string, error) // for recursive loops
	undefinedBehavior UndefinedBehavior
	temps             *tempStore
	fuelTracker       *fuelTracker
	rootContext       value.Value
}

type outputWriter interface {
	WriteString(string) (int, error)
}

type ioStringWriter struct {
	w io.Writer
}

func (w ioStringWriter) WriteString(s string) (int, error) {
	return io.WriteString(w.w, s)
}

// Context returns the context.Context associated with this rendering operation.
//
// This context can be used for cancellation, timeouts, and passing request-scoped
// values to custom filters and functions.
//
// Example usage in a custom filter:
//
//	env.AddFilter("fetch_data", func(state FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
//	    ctx := state.Context()
//	    // Use ctx for database queries, HTTP requests, etc.
//	    select {
//	    case <-ctx.Done():
//	        return value.Undefined(), ctx.Err()
//	    default:
//	        // Continue processing
//	    }
//	    return value.FromString("result"), nil
//	})
func (s *State) Context() context.Context {
	if s.ctx == nil {
		return context.Background()
	}
	return s.ctx
}

// Env returns the Environment associated with this rendering operation.
//
// This can be used by custom filters and functions to access environment
// configuration or load additional templates.
func (s *State) Env() *Environment {
	return s.env
}

// GetFilter returns a registered filter by name.
func (s *State) GetFilter(name string) (FilterFunc, bool) {
	return s.env.getFilter(name)
}

// GetTest returns a registered test by name.
func (s *State) GetTest(name string) (TestFunc, bool) {
	return s.env.getTest(name)
}

// Name returns the name of the template currently being rendered.
func (s *State) Name() string {
	return s.name
}

// AutoEscape returns the current auto-escape setting for this template.
func (s *State) AutoEscape() AutoEscape {
	return s.autoEscape
}

// FuelLevels returns the consumed and remaining fuel if fuel tracking is enabled.
func (s *State) FuelLevels() (consumed, remaining uint64, ok bool) {
	if s.fuelTracker == nil {
		return 0, 0, false
	}
	return s.fuelTracker.consumedFuel(), s.fuelTracker.remainingFuel(), true
}

// UndefinedBehavior returns the undefined behavior setting for this template.
func (s *State) UndefinedBehavior() UndefinedBehavior {
	return s.undefinedBehavior
}

func (s *State) isStrictUndefined() bool {
	return s.undefinedBehavior == UndefinedStrict
}

func (s *State) isStrictOrSemiStrict() bool {
	return s.undefinedBehavior == UndefinedStrict || s.undefinedBehavior == UndefinedSemiStrict
}

func (s *State) handleUndefined(parentWasUndefined bool) (value.Value, error) {
	switch s.undefinedBehavior {
	case UndefinedChainable:
		return value.Undefined(), nil
	case UndefinedLenient, UndefinedSemiStrict, UndefinedStrict:
		if parentWasUndefined {
			return value.Undefined(), NewError(ErrUndefinedVar, "undefined value")
		}
		return value.Undefined(), nil
	default:
		return value.Undefined(), nil
	}
}

func (s *State) isTrue(val value.Value) (bool, error) {
	if val.IsUndefined() && s.isStrictUndefined() && !val.IsSilentUndefined() {
		return false, NewError(ErrUndefinedVar, "undefined value")
	}
	return val.IsTrue(), nil
}

func (s *State) assertIterable(val value.Value) error {
	if val.IsUndefined() && s.isStrictOrSemiStrict() && !val.IsSilentUndefined() {
		return NewError(ErrUndefinedVar, "undefined value")
	}
	return nil
}

// RenderBlock renders a specific block by name.
//
// This is useful for rendering parts of a template, such as for
// AJAX responses or email subjects. Only available after EvalToState.
//
// Example:
//
//	state, _ := tmpl.EvalToState(ctx)
//	title, err := state.RenderBlock("title")
func (s *State) RenderBlock(name string) (string, error) {
	bs := s.blocks[name]
	if bs == nil || len(bs.layers) == 0 {
		return "", NewError(ErrInvalidOperation, "block not found: "+name)
	}

	// Capture output
	oldOut := s.out
	builder := &strings.Builder{}
	s.out = builder

	oldBlock := s.currentBlock
	s.currentBlock = name

	s.pushScope()
	for _, stmt := range bs.layers[0] {
		if err := s.evalStmt(stmt); err != nil {
			s.popScope()
			s.currentBlock = oldBlock
			s.out = oldOut
			return "", err
		}
	}
	s.popScope()

	result := builder.String()
	s.out = oldOut
	s.currentBlock = oldBlock

	return result, nil
}

// CallMacro calls a macro by name with the given arguments.
//
// Example:
//
//	state, _ := tmpl.EvalToState(ctx)
//	result, err := state.CallMacro("render_item", value.FromString("hello"))
func (s *State) CallMacro(name string, args ...value.Value) (value.Value, error) {
	macro, ok := s.macros[name]
	if !ok {
		return value.Undefined(), NewError(ErrInvalidOperation, "macro not found: "+name)
	}
	return newMacroCallableFromDefinition(macro, value.Undefined()).Call(s, args, nil)
}

// CallMacroKw calls a macro by name with positional and keyword arguments.
//
// Example:
//
//	state, _ := tmpl.EvalToState(ctx)
//	result, err := state.CallMacroKw("render_input",
//	    []value.Value{value.FromString("username")},
//	    map[string]value.Value{"type": value.FromString("email")},
//	)
func (s *State) CallMacroKw(name string, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	macro, ok := s.macros[name]
	if !ok {
		return value.Undefined(), NewError(ErrInvalidOperation, "macro not found: "+name)
	}
	return newMacroCallableFromDefinition(macro, value.Undefined()).Call(s, args, kwargs)
}

// Exports returns all exported variables from the template.
//
// Exports are variables set at the top level of the template using {% set %}.
// This also includes macros defined in the template.
//
// Example:
//
//	state, _ := tmpl.EvalToState(ctx)
//	exports := state.Exports()
//	for name, val := range exports {
//	    fmt.Printf("%s = %v\n", name, val)
//	}
func (s *State) Exports() map[string]value.Value {
	exports := make(map[string]value.Value)

	// Get variables from the top-level scope
	if len(s.scopes) > 0 {
		for k, v := range s.scopes[0] {
			exports[k] = v
		}
	}

	if !s.rootContext.IsUndefined() {
		if obj, ok := s.rootContext.AsObject(); ok {
			if m, ok := obj.(value.MapObject); ok {
				for _, key := range m.Keys() {
					exports[key] = s.rootContext.GetAttr(key)
				}
			}
		}
	}

	// Also include macros as exports
	for name, macro := range s.macros {
		exports[name] = value.FromCallable(newMacroCallableFromDefinition(macro, value.Undefined()))
	}

	return exports
}

// KnownVariables returns a list of all known variables in the current state.
//
// This includes variables from all scopes, macros, and globals/functions from
// the environment. This is mostly useful for introspection.
func (s *State) KnownVariables() []string {
	seen := make(map[string]struct{})
	for i := len(s.scopes) - 1; i >= 0; i-- {
		for key := range s.scopes[i] {
			if _, ok := seen[key]; !ok {
				seen[key] = struct{}{}
			}
		}
	}
	if !s.rootContext.IsUndefined() {
		if obj, ok := s.rootContext.AsObject(); ok {
			if m, ok := obj.(value.MapObject); ok {
				for _, key := range m.Keys() {
					if _, ok := seen[key]; !ok {
						seen[key] = struct{}{}
					}
				}
			}
		}
	}
	for name := range s.macros {
		if _, ok := seen[name]; !ok {
			seen[name] = struct{}{}
		}
	}
	for name := range s.env.globals {
		if _, ok := seen[name]; !ok {
			seen[name] = struct{}{}
		}
	}
	for name := range s.env.functions {
		if _, ok := seen[name]; !ok {
			seen[name] = struct{}{}
		}
	}

	vars := make([]string, 0, len(seen))
	for name := range seen {
		vars = append(vars, name)
	}
	sort.Strings(vars)
	return vars
}

// BlockNames returns the names of all blocks defined in the template.
func (s *State) BlockNames() []string {
	names := make([]string, 0, len(s.blocks))
	for name := range s.blocks {
		names = append(names, name)
	}
	return names
}

// MacroNames returns the names of all macros defined in the template.
func (s *State) MacroNames() []string {
	names := make([]string, 0, len(s.macros))
	for name := range s.macros {
		names = append(names, name)
	}
	return names
}

// CurrentBlock returns the name of the block currently being rendered.
// Returns empty string if not inside a block.
func (s *State) CurrentBlock() string {
	return s.currentBlock
}

// RenderBlockToWrite renders a specific block and writes the output to the given writer.
func (s *State) RenderBlockToWrite(name string, w io.Writer) error {
	result, err := s.RenderBlock(name)
	if err != nil {
		return err
	}
	_, err = io.WriteString(w, result)
	return err
}

// ApplyFilter invokes a filter with the given value and arguments.
//
// Example:
//
//	result, err := state.ApplyFilter("upper", value.FromString("hello"), nil, nil)
//	// result is "HELLO"
func (s *State) ApplyFilter(name string, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	filterFn, ok := s.env.getFilter(name)
	if !ok {
		return value.Undefined(), NewError(ErrUnknownFilter, name)
	}
	return filterFn(s, val, args, kwargs)
}

// PerformTest invokes a test with the given value and arguments.
//
// Example:
//
//	result, err := state.PerformTest("even", value.FromInt(42), nil)
//	// result is true
func (s *State) PerformTest(name string, val value.Value, args []value.Value) (bool, error) {
	testFn, ok := s.env.getTest(name)
	if !ok {
		return false, NewError(ErrUnknownTest, name)
	}
	return testFn(s, val, args)
}

// Format renders a value using the environment formatter and current auto-escape.
//
// This mirrors how values are written during template rendering, but returns
// the resulting string instead of writing to the output.
func (s *State) Format(val value.Value) (string, error) {
	return s.formatValue(val, true)
}

// GetTemp retrieves a temporary value stored in the state.
//
// Temps are not scoped and exist for the lifetime of the render operation.
// They are useful for sharing state between filters or functions.
func (s *State) GetTemp(name string) (value.Value, bool) {
	if s.temps == nil {
		return value.Undefined(), false
	}
	s.temps.mu.Lock()
	defer s.temps.mu.Unlock()
	v, ok := s.temps.values[name]
	return v, ok
}

// SetTemp stores a temporary value and returns the previous value if present.
//
// For more information see GetTemp.
func (s *State) SetTemp(name string, val value.Value) (value.Value, bool) {
	if s.temps == nil {
		s.temps = &tempStore{values: make(map[string]value.Value)}
	}
	s.temps.mu.Lock()
	defer s.temps.mu.Unlock()
	old, ok := s.temps.values[name]
	s.temps.values[name] = val
	return old, ok
}

// GetTemplate fetches a template by name from the environment.
// This is a convenience method equivalent to state.Env().GetTemplate(name).
func (s *State) GetTemplate(name string) (*Template, error) {
	return s.env.GetTemplate(name)
}

type tempStore struct {
	mu     sync.Mutex
	values map[string]value.Value
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

func (m *macroCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state // macro uses its captured state
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

func (f *functionCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state // function uses its captured state
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
	if s.depth > s.recursionLimit() {
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
	builder := &strings.Builder{}
	s.out = builder
	for _, stmt := range macro.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return value.Undefined(), err
		}
	}
	result := builder.String()
	s.out = oldOut

	return value.FromSafeString(result), nil
}

// loopObject is the loop variable object that supports cycle() and previtem/nextitem
type loopObject struct {
	index      int                // 0-based index
	length     int                // total length (-1 for unknown)
	depth      int                // nesting depth (0-based)
	items      []value.Value      // all items for previtem/nextitem
	changed    *value.Value       // last value for changed()
	prevItem   value.Value        // previous item (for pull iterators)
	pullIter   value.PullIterator // pull-based iterator
	peekedNext *value.Value       // peeked next item for pullIter
	recurseFn  func(value.Value) (string, error)
}

func (l *loopObject) GetAttr(name string) value.Value {
	switch name {
	case "index":
		return value.FromInt(int64(l.index + 1))
	case "index0":
		return value.FromInt(int64(l.index))
	case "revindex":
		if l.length < 0 {
			return value.Undefined()
		}
		return value.FromInt(int64(l.length - l.index))
	case "revindex0":
		if l.length < 0 {
			return value.Undefined()
		}
		return value.FromInt(int64(l.length - l.index - 1))
	case "first":
		return value.FromBool(l.index == 0)
	case "last":
		if l.length < 0 {
			return value.Undefined()
		}
		return value.FromBool(l.index == l.length-1)
	case "length":
		if l.length < 0 {
			return value.Undefined()
		}
		return value.FromInt(int64(l.length))
	case "depth":
		return value.FromInt(int64(l.depth + 1))
	case "depth0":
		return value.FromInt(int64(l.depth))
	case "previtem":
		if l.pullIter != nil {
			return l.prevItem
		}
		if l.index > 0 {
			return l.items[l.index-1]
		}
		return value.Undefined()
	case "nextitem":
		if l.pullIter != nil {
			// For pull iterators, we need to peek the next item
			// Note: This consumes the item from the pull iterator!
			if l.peekedNext == nil {
				next, ok := l.pullIter.PullNext()
				if ok {
					l.peekedNext = &next
				}
			}
			if l.peekedNext != nil {
				return *l.peekedNext
			}
			return value.Undefined()
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

func (l *loopObject) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
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

func (c *loopCycleCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
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

func (c *loopChangedCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
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

func (bc *blockCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
	bs := bc.state.blocks[bc.blockName]
	if bs == nil || len(bs.layers) == 0 {
		return value.Undefined(), NewError(ErrInvalidOperation, fmt.Sprintf("block '%s' not found", bc.blockName))
	}

	// Capture output
	oldOut := bc.state.out
	builder := &strings.Builder{}
	bc.state.out = builder

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

	result := builder.String()
	bc.state.out = oldOut
	bc.state.currentBlock = oldBlock

	return value.FromSafeString(result), nil
}

const maxRecursion = 500

func newState(goCtx context.Context, env *Environment, name, source string, ctx value.Value) *State {
	// Initialize root scope with context
	rootScope := make(map[string]value.Value)
	rootContext := value.Undefined()
	if m, ok := ctx.AsMap(); ok {
		for k, v := range m {
			rootScope[k] = v
		}
	} else if obj, ok := ctx.AsObject(); ok {
		if _, ok := obj.(value.MapObject); ok || value.GetObjectRepr(obj) == value.ObjectReprMap {
			rootContext = ctx
		}
	}

	var tracker *fuelTracker
	if env != nil && env.fuel != nil {
		tracker = newFuelTracker(*env.fuel)
	}

	return &State{
		ctx:               goCtx,
		env:               env,
		name:              name,
		source:            source,
		autoEscape:        env.autoEscapeFunc(name),
		scopes:            []map[string]value.Value{rootScope},
		blocks:            make(map[string]*blockStack),
		macros:            make(map[string]*macroDefinition),
		out:               &strings.Builder{},
		undefinedBehavior: env.undefinedBehavior,
		temps:             &tempStore{values: make(map[string]value.Value)},
		fuelTracker:       tracker,
		rootContext:       rootContext,
	}
}

func (s *State) outputString() string {
	if b, ok := s.out.(*strings.Builder); ok {
		return b.String()
	}
	return ""
}

func (s *State) consumeFuel() error {
	if s.fuelTracker == nil {
		return nil
	}
	return s.fuelTracker.consume(1)
}

func (s *State) recursionLimit() int {
	if s.env == nil {
		return maxRecursion
	}
	return s.env.recursionLimit
}

func (s *State) decorateError(err error) error {
	return s.attachErrorInfo(err, nil)
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

	if !s.rootContext.IsUndefined() {
		if v := s.rootContext.GetAttr(name); !v.IsUndefined() {
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
				return "", s.decorateError(NewError(ErrInvalidOperation, "tried to extend a second time in a template"))
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
					return "", s.decorateError(err)
				}
			case *parser.SetBlock:
				if err := s.evalSetBlock(st); err != nil {
					return "", s.decorateError(err)
				}
			case *parser.Import:
				if err := s.evalImport(st); err != nil {
					return "", s.decorateError(err)
				}
			case *parser.FromImport:
				if err := s.evalFromImport(st); err != nil {
					return "", s.decorateError(err)
				}
			}
		}

		// Now process extends
		if err := s.evalExtends(extendsStmt); err != nil && err != errExtendsExecuted {
			return "", s.decorateError(err)
		}
		return s.outputString(), nil
	}

	// Non-extending template - evaluate normally
	for _, stmt := range tmpl.Children {
		if err := s.evalStmt(stmt); err != nil {
			return "", s.decorateError(err)
		}
	}
	return s.outputString(), nil
}

func (s *State) evalStmt(stmt parser.Stmt) (err error) {
	if s.env != nil && s.env.debug {
		defer func() {
			if err != nil {
				err = s.attachErrorInfo(err, stmt)
			}
		}()
	}
	if err = s.consumeFuel(); err != nil {
		return err
	}

	switch st := stmt.(type) {
	case *parser.EmitRaw:
		if _, err := s.out.WriteString(st.Raw); err != nil {
			return err
		}
		return nil

	case *parser.EmitExpr:
		val, err := s.evalExpr(st.Expr)
		if err != nil {
			return err
		}
		return s.writeValue(val)

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

	// Check for PullIterator (new one-shot iterator interface)
	if obj, ok := iter.AsObject(); ok {
		if pull, ok := obj.(value.PullIterator); ok {
			if loop.Recursive || loop.FilterExpr != nil {
				// Need to collect all items for recursive/filtered loops
				var items []value.Value
				for {
					v, ok := pull.PullNext()
					if !ok {
						break
					}
					items = append(items, v)
				}
				return s.evalForLoopItems(loop, items)
			}
			return s.evalForLoopPull(loop, pull)
		}
	}

	items := iter.Iter()
	if items == nil {
		if iter.IsUndefined() || iter.IsNone() {
			if err := s.assertIterable(iter); err != nil {
				return err
			}
			items = []value.Value{}
		} else {
			return NewError(ErrInvalidOperation, fmt.Sprintf("%s is not iterable", iter.Kind()))
		}
	}

	return s.evalForLoopItems(loop, items)
}

// evalForLoopPull handles iteration over PullIterator (new one-shot iterator interface).
// Unlike evalForLoopItems, this pulls items one at a time, allowing partial consumption.
func (s *State) evalForLoopPull(loop *parser.ForLoop, pull value.PullIterator) error {
	// Check if iterator is empty first
	if pull.PullDone() {
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
	if s.depth > s.recursionLimit() {
		return NewError(ErrInvalidOperation, "recursion limit exceeded")
	}

	s.pushScope()
	defer func() {
		s.popScope()
		s.depth--
	}()

	index := 0
	prevItem := value.Undefined()
	var peekedItem *value.Value // Track peeked item across loop iterations

	for {
		var item value.Value
		var ok bool

		// If we have a peeked item from loop.nextitem, use it
		if peekedItem != nil {
			item = *peekedItem
			peekedItem = nil
			ok = true
		} else {
			item, ok = pull.PullNext()
		}

		if !ok {
			break
		}
		if err := s.unpackLoopTarget(loop.Target, item); err != nil {
			return err
		}

		// For pull iterators, length is unknown (-1)
		loopObj := &loopObject{
			index:     index,
			length:    -1, // Unknown length
			depth:     s.depth - 1,
			items:     nil,
			prevItem:  prevItem,
			pullIter:  pull,
			recurseFn: s.loopRecurse,
		}
		s.Set("loop", value.FromObject(loopObj))

		for _, stmt := range loop.Body {
			err := s.evalStmt(stmt)
			if err == errContinue {
				break
			}
			if err == errBreak {
				// Items remain in the pull iterator for potential future use
				return nil
			}
			if err != nil {
				return err
			}
		}

		// If loop.nextitem was accessed, it consumed the next item
		// We need to capture it for the next iteration
		if loopObj.peekedNext != nil {
			peekedItem = loopObj.peekedNext
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
			truthy, err := s.isTrue(cond)
			if err != nil {
				s.popScope()
				return wrapEvalError(err, loop.FilterExpr.Span())
			}
			if truthy {
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
	if s.depth > s.recursionLimit() {
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
			if s.depth > s.recursionLimit() {
				return "", NewError(ErrInvalidOperation, "recursion limit exceeded")
			}
			defer func() { s.depth-- }()

			s.pushScope()
			defer s.popScope()

			nestedItems := iterValue.Iter()
			if nestedItems == nil {
				if iterValue.IsUndefined() || iterValue.IsNone() {
					if err := s.assertIterable(iterValue); err != nil {
						return "", err
					}
					return "", nil
				}
				return "", NewError(ErrInvalidOperation, "cannot recurse because of non-iterable value")
			}

			oldOut := s.out
			builder := &strings.Builder{}
			s.out = builder

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
						result := builder.String()
						s.out = oldOut
						return result, nil
					}
					if err != nil {
						s.out = oldOut
						return "", err
					}
				}
			}

			result := builder.String()
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

	truthy, err := s.isTrue(val)
	if err != nil {
		return wrapEvalError(err, cond.Span())
	}

	if truthy {
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
	builder := &strings.Builder{}
	s.out = builder
	for _, stmt := range block.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return err
		}
	}
	captured := builder.String()
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

	resolvedName := s.env.joinTemplatePath(name, s.name)

	// Load the parent template
	parentTmpl, err := s.env.GetTemplate(resolvedName)
	if err != nil {
		return err
	}

	s.depth++
	if s.depth > s.recursionLimit() {
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
			if isTemplateNotFound(err) {
				return err
			}
			return wrapIncludeError(err, inc.Span(), name)
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
				if isTemplateNotFound(err) {
					lastErr = err
					continue
				}
				return wrapIncludeError(err, inc.Span(), name)
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
	resolvedName := s.env.joinTemplatePath(name, s.name)
	tmpl, err := s.env.GetTemplate(resolvedName)
	if err != nil {
		return err
	}

	s.depth++
	if s.depth > s.recursionLimit() {
		return NewError(ErrInvalidOperation, "recursion limit exceeded")
	}

	// Create new state with isolated scope
	childState := &State{
		ctx:               s.ctx,
		env:               s.env,
		name:              tmpl.compiled.name,
		source:            tmpl.compiled.source,
		autoEscape:        s.env.autoEscapeFunc(tmpl.compiled.name),
		scopes:            cloneScopes(s.scopes),
		blocks:            s.blocks,
		macros:            s.macros,
		out:               s.out,
		depth:             s.depth,
		undefinedBehavior: s.undefinedBehavior,
		temps:             s.temps,
		fuelTracker:       s.fuelTracker,
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

func wrapIncludeError(err error, span parser.Span, name string) error {
	tmplName := name
	if templErr, ok := err.(*Error); ok && templErr.Name != "" {
		tmplName = templErr.Name
	}
	return NewError(ErrBadInclude, fmt.Sprintf("error in %q", tmplName)).WithSpan(span).WithCause(err)
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

	resolvedPath := s.env.joinTemplatePath(path, s.name)

	// Load and parse the template
	tmpl, err := s.env.GetTemplate(resolvedPath)
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

	resolvedPath := s.env.joinTemplatePath(path, s.name)

	// Load and parse the template
	tmpl, err := s.env.GetTemplate(resolvedPath)
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
		ctx:               s.ctx,
		env:               s.env,
		name:              tmpl.name,
		source:            tmpl.source,
		autoEscape:        s.env.autoEscapeFunc(tmpl.name),
		scopes:            cloneScopes(s.scopes),
		blocks:            make(map[string]*blockStack),
		macros:            make(map[string]*macroDefinition),
		out:               &strings.Builder{},
		depth:             s.depth,
		undefinedBehavior: s.undefinedBehavior,
		temps:             s.temps,
		fuelTracker:       s.fuelTracker,
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
	builder := &strings.Builder{}
	s.out = builder
	for _, stmt := range block.Body {
		if err := s.evalStmt(stmt); err != nil {
			s.out = oldOut
			return err
		}
	}
	captured := builder.String()
	s.out = oldOut

	capturedVal := value.FromString(captured)
	if !oldEscape.IsNone() {
		capturedVal = value.FromSafeString(captured)
	}

	result, err := s.applyFilter(block.Filter, capturedVal)
	if err != nil {
		return err
	}

	if err := s.writeValue(result); err != nil {
		return err
	}
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
			if oldEscape.IsNone() {
				s.autoEscape = AutoEscapeHTML
			} else {
				s.autoEscape = oldEscape
			}
		} else {
			s.autoEscape = AutoEscapeNone
		}
	} else if str, ok := val.AsString(); ok {
		switch str {
		case "html":
			s.autoEscape = AutoEscapeHTML
		case "json":
			s.autoEscape = AutoEscapeJSON
		case "none":
			s.autoEscape = AutoEscapeNone
		default:
			return NewError(ErrInvalidOperation, "invalid value to autoescape tag").WithSpan(ae.Span())
		}
	} else {
		return NewError(ErrInvalidOperation, "invalid value to autoescape tag").WithSpan(ae.Span())
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
	result, err := newMacroCallableFromDefinition(macroDef, value.FromCallable(callerCallable)).Call(s, args, kwargs)
	if err != nil {
		return err
	}

	if err := s.writeValue(result); err != nil {
		return err
	}
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

func (c *callerCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
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
	builder := &strings.Builder{}
	c.state.out = builder

	for _, stmt := range c.body {
		if err := c.state.evalStmt(stmt); err != nil {
			c.state.out = oldOut
			return value.Undefined(), err
		}
	}

	result := builder.String()
	c.state.out = oldOut

	return value.FromSafeString(result), nil
}

func (s *State) writeValue(val value.Value) error {
	str, err := s.formatValue(val, true)
	if err != nil {
		return err
	}
	_, err = s.out.WriteString(str)
	return err
}

func (s *State) formatValue(val value.Value, strictUndefined bool) (string, error) {
	if val.IsUndefined() {
		if strictUndefined && s.isStrictOrSemiStrict() && !val.IsSilentUndefined() {
			return "", NewError(ErrUndefinedVar, "undefined value")
		}
		return "", nil
	}

	// Use custom formatter if set
	if s.env.formatter != nil {
		escape := func(str string) string {
			if s.autoEscape.IsHTML() {
				return EscapeHTML(str)
			}
			return str
		}
		return s.env.formatter(s, val, escape), nil
	}

	if val.IsSafe() {
		return val.String(), nil
	}

	switch {
	case s.autoEscape.IsNone():
		return val.String(), nil
	case s.autoEscape.IsHTML():
		return EscapeHTML(val.String()), nil
	case s.autoEscape.IsJSON():
		return formatJSONValue(val)
	case s.autoEscape.IsCustom():
		return "", NewError(ErrInvalidOperation, fmt.Sprintf("Default formatter does not know how to format to custom format '%s'", s.autoEscape.CustomName()))
	default:
		return val.String(), nil
	}
}

func formatJSONValue(val value.Value) (string, error) {
	native := valueToNative(val)
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	if err := enc.Encode(native); err != nil {
		return "", NewError(ErrInvalidOperation, "unable to format to JSON")
	}
	out := buf.String()
	out = strings.TrimSuffix(out, "\n")
	return out, nil
}

func (s *State) evalExpr(expr parser.Expr) (rv value.Value, err error) {
	if s.env != nil && s.env.debug {
		defer func() {
			if err != nil {
				err = s.attachErrorInfo(err, expr)
			}
		}()
	}
	if err = s.consumeFuel(); err != nil {
		return value.Undefined(), err
	}

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
	case *parser.BigInt:
		return value.FromBigInt(v.Int)
	default:
		return value.FromAny(v)
	}
}

func wrapEvalError(err error, span parser.Span) error {
	if err == nil {
		return nil
	}
	if templErr, ok := err.(*Error); ok {
		if templErr.Span == nil {
			templErr.WithSpan(span)
		}
		return templErr
	}
	return NewError(ErrInvalidOperation, err.Error()).WithSpan(span)
}

func (s *State) evalUnaryOp(op *parser.UnaryOp) (value.Value, error) {
	val, err := s.evalExpr(op.Expr)
	if err != nil {
		return value.Undefined(), err
	}

	switch op.Op {
	case parser.UnaryNot:
		truthy, err := s.isTrue(val)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return value.FromBool(!truthy), nil
	case parser.UnaryNeg:
		rv, err := val.Neg()
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	default:
		return value.Undefined(), NewError(ErrInvalidOperation, "unknown unary operator").WithSpan(op.Span())
	}
}

func (s *State) evalBinOp(op *parser.BinOp) (value.Value, error) {
	// Short-circuit evaluation for and/or
	if op.Op == parser.BinOpScAnd {
		left, err := s.evalExpr(op.Left)
		if err != nil {
			return value.Undefined(), err
		}
		truthy, err := s.isTrue(left)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		if !truthy {
			return left, nil
		}
		return s.evalExpr(op.Right)
	}

	if op.Op == parser.BinOpScOr {
		left, err := s.evalExpr(op.Left)
		if err != nil {
			return value.Undefined(), err
		}
		truthy, err := s.isTrue(left)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		if truthy {
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
		return value.Undefined(), NewError(ErrInvalidOperation, fmt.Sprintf("cannot compare %s and %s", left.Kind(), right.Kind())).WithSpan(op.Span())
	case parser.BinOpLte:
		if cmp, ok := left.Compare(right); ok {
			return value.FromBool(cmp <= 0), nil
		}
		return value.Undefined(), NewError(ErrInvalidOperation, fmt.Sprintf("cannot compare %s and %s", left.Kind(), right.Kind())).WithSpan(op.Span())
	case parser.BinOpGt:
		if cmp, ok := left.Compare(right); ok {
			return value.FromBool(cmp > 0), nil
		}
		return value.Undefined(), NewError(ErrInvalidOperation, fmt.Sprintf("cannot compare %s and %s", left.Kind(), right.Kind())).WithSpan(op.Span())
	case parser.BinOpGte:
		if cmp, ok := left.Compare(right); ok {
			return value.FromBool(cmp >= 0), nil
		}
		return value.Undefined(), NewError(ErrInvalidOperation, fmt.Sprintf("cannot compare %s and %s", left.Kind(), right.Kind())).WithSpan(op.Span())
	case parser.BinOpAdd:
		rv, err := left.Add(right)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	case parser.BinOpSub:
		rv, err := left.Sub(right)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	case parser.BinOpMul:
		rv, err := left.Mul(right)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	case parser.BinOpDiv:
		rv, err := left.Div(right)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	case parser.BinOpFloorDiv:
		rv, err := left.FloorDiv(right)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	case parser.BinOpRem:
		rv, err := left.Rem(right)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	case parser.BinOpPow:
		rv, err := left.Pow(right)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return rv, nil
	case parser.BinOpConcat:
		return left.Concat(right), nil
	case parser.BinOpIn:
		if err := s.assertIterable(right); err != nil {
			return value.Undefined(), wrapEvalError(err, op.Span())
		}
		return value.FromBool(right.Contains(left)), nil
	default:
		return value.Undefined(), NewError(ErrInvalidOperation, fmt.Sprintf("unknown binary operator: %v", op.Op)).WithSpan(op.Span())
	}
}

func (s *State) evalIfExpr(ie *parser.IfExpr) (value.Value, error) {
	cond, err := s.evalExpr(ie.TestExpr)
	if err != nil {
		return value.Undefined(), err
	}

	truthy, err := s.isTrue(cond)
	if err != nil {
		return value.Undefined(), wrapEvalError(err, ie.Span())
	}

	if truthy {
		return s.evalExpr(ie.TrueExpr)
	}

	if ie.FalseExpr != nil {
		return s.evalExpr(ie.FalseExpr)
	}
	return value.SilentUndefined(), nil
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
		val, err := s.handleUndefined(true)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, ga.Span())
		}
		return val, nil
	}
	return val.GetAttr(ga.Name), nil
}

func (s *State) evalGetItem(gi *parser.GetItem) (value.Value, error) {
	val, err := s.evalExpr(gi.Expr)
	if err != nil {
		return value.Undefined(), err
	}
	if val.IsUndefined() {
		val, err := s.handleUndefined(true)
		if err != nil {
			return value.Undefined(), wrapEvalError(err, gi.Span())
		}
		return val, nil
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
			return s.evalSuper(call.Span())
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
			return newMacroCallableFromDefinition(macro, value.Undefined()).Call(s, args, kwargs)
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
			return callable.Call(s, args, kwargs)
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
		return callable.Call(s, args, kwargs)
	}

	// Check if it's a CallableObject (object that can be called directly)
	if obj, ok := expr.AsObject(); ok {
		if co, ok := obj.(value.CallableObject); ok {
			args, kwargs, err := s.evalCallArgs(call.Args)
			if err != nil {
				return value.Undefined(), err
			}
			return co.ObjectCall(s, args, kwargs)
		}
	}

	// Check if it's a method call on a map (like module.macro())
	if getAttr, ok := call.Expr.(*parser.GetAttr); ok {
		obj, err := s.evalExpr(getAttr.Expr)
		if err != nil {
			return value.Undefined(), err
		}

		// Check if object supports method calls directly
		if objVal, ok := obj.AsObject(); ok {
			if mc, ok := objVal.(value.MethodCallable); ok {
				args, kwargs, err := s.evalCallArgs(call.Args)
				if err != nil {
					return value.Undefined(), err
				}
				result, err := mc.CallMethod(s, getAttr.Name, args, kwargs)
				if err != value.ErrUnknownMethod {
					return result, err
				}
				// Fall through to try GetAttr
			}
		}

		attr := obj.GetAttr(getAttr.Name)
		if callable, ok := attr.AsCallable(); ok {
			args, kwargs, err := s.evalCallArgs(call.Args)
			if err != nil {
				return value.Undefined(), err
			}
			return callable.Call(s, args, kwargs)
		}
	}

	return value.Undefined(), NewError(ErrUnknownFunction, "unknown callable").WithSpan(call.Span())
}

func (s *State) evalSuper(span parser.Span) (value.Value, error) {
	if s.currentBlock == "" {
		return value.Undefined(), NewError(ErrInvalidOperation, "super() can only be used inside a block").WithSpan(span)
	}

	bs := s.blocks[s.currentBlock]
	if bs == nil || bs.index+1 >= len(bs.layers) {
		return value.Undefined(), NewError(ErrInvalidOperation, "no parent block exists").WithSpan(span)
	}

	// Move to the parent block
	bs.index++
	defer func() { bs.index-- }()

	// Capture output
	oldOut := s.out
	builder := &strings.Builder{}
	s.out = builder

	s.pushScope()
	for _, stmt := range bs.layers[bs.index] {
		if err := s.evalStmt(stmt); err != nil {
			s.popScope()
			s.out = oldOut
			return value.Undefined(), NewError(ErrEvalBlock, "error in super block").WithSpan(span).WithCause(err)
		}
	}
	s.popScope()

	result := builder.String()
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
