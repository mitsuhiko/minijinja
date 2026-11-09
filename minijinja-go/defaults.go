package minijinja

import (
	"fmt"
	"sort"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/filters"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/tests"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

func registerDefaultFilters(env *Environment) {
	// String filters
	env.AddFilter("upper", filters.FilterUpper)
	env.AddFilter("lower", filters.FilterLower)
	env.AddFilter("capitalize", filters.FilterCapitalize)
	env.AddFilter("title", filters.FilterTitle)
	env.AddFilter("trim", filters.FilterTrim)
	env.AddFilter("replace", filters.FilterReplace)
	env.AddFilter("format", filters.FilterFormat)
	env.AddFilter("default", filters.FilterDefault)
	env.AddFilter("d", filters.FilterDefault) // alias
	env.AddFilter("safe", filters.FilterSafe)
	env.AddFilter("escape", filters.FilterEscape)
	env.AddFilter("e", filters.FilterEscape) // alias
	env.AddFilter("string", filters.FilterString)
	env.AddFilter("bool", filters.FilterBool)
	env.AddFilter("split", filters.FilterSplit)
	env.AddFilter("lines", filters.FilterLines)

	// List/sequence filters
	env.AddFilter("length", filters.FilterLength)
	env.AddFilter("count", filters.FilterLength) // alias
	env.AddFilter("first", filters.FilterFirst)
	env.AddFilter("last", filters.FilterLast)
	env.AddFilter("reverse", filters.FilterReverse)
	env.AddFilter("sort", filters.FilterSort)
	env.AddFilter("join", filters.FilterJoin)
	env.AddFilter("list", filters.FilterList)
	env.AddFilter("unique", filters.FilterUnique)
	env.AddFilter("min", filters.FilterMin)
	env.AddFilter("max", filters.FilterMax)
	env.AddFilter("sum", filters.FilterSum)
	env.AddFilter("batch", filters.FilterBatch)
	env.AddFilter("slice", filters.FilterSlice)
	env.AddFilter("map", filters.FilterMap)
	env.AddFilter("select", filters.FilterSelect)
	env.AddFilter("reject", filters.FilterReject)
	env.AddFilter("selectattr", filters.FilterSelectAttr)
	env.AddFilter("rejectattr", filters.FilterRejectAttr)
	env.AddFilter("groupby", filters.FilterGroupBy)
	env.AddFilter("chain", filters.FilterChain)
	env.AddFilter("zip", filters.FilterZip)

	// Numeric filters
	env.AddFilter("abs", filters.FilterAbs)
	env.AddFilter("int", filters.FilterInt)
	env.AddFilter("float", filters.FilterFloat)
	env.AddFilter("round", filters.FilterRound)

	// Dict filters
	env.AddFilter("items", filters.FilterItems)
	env.AddFilter("dictsort", filters.FilterDictSort)

	// Other filters
	env.AddFilter("attr", filters.FilterAttr)
	env.AddFilter("indent", filters.FilterIndent)
	env.AddFilter("pprint", filters.FilterPprint)

	// JSON and URL filters
	env.AddFilter("tojson", filters.FilterTojson)
	env.AddFilter("urlencode", filters.FilterUrlencode)
}

func registerDefaultTests(env *Environment) {
	env.AddTest("defined", tests.TestDefined)
	env.AddTest("undefined", tests.TestUndefined)
	env.AddTest("none", tests.TestNone)
	env.AddTest("true", tests.TestTrue)
	env.AddTest("false", tests.TestFalse)
	env.AddTest("odd", tests.TestOdd)
	env.AddTest("even", tests.TestEven)
	env.AddTest("divisibleby", tests.TestDivisibleBy)
	env.AddTest("eq", tests.TestEq)
	env.AddTest("equalto", tests.TestEq)
	env.AddTest("==", tests.TestEq)
	env.AddTest("ne", tests.TestNe)
	env.AddTest("!=", tests.TestNe)
	env.AddTest("lt", tests.TestLt)
	env.AddTest("lessthan", tests.TestLt)
	env.AddTest("<", tests.TestLt)
	env.AddTest("le", tests.TestLe)
	env.AddTest("<=", tests.TestLe)
	env.AddTest("gt", tests.TestGt)
	env.AddTest("greaterthan", tests.TestGt)
	env.AddTest(">", tests.TestGt)
	env.AddTest("ge", tests.TestGe)
	env.AddTest(">=", tests.TestGe)
	env.AddTest("in", tests.TestIn)
	env.AddTest("string", tests.TestString)
	env.AddTest("number", tests.TestNumber)
	env.AddTest("integer", tests.TestInteger)
	env.AddTest("int", tests.TestInteger) // alias
	env.AddTest("float", tests.TestFloat)
	env.AddTest("boolean", tests.TestBoolean)
	env.AddTest("sequence", tests.TestSequence)
	env.AddTest("mapping", tests.TestMapping)
	env.AddTest("iterable", tests.TestIterable)
	env.AddTest("startingwith", tests.TestStartingWith)
	env.AddTest("endingwith", tests.TestEndingWith)
	env.AddTest("containing", tests.TestContaining)
	env.AddTest("safe", tests.TestSafe)
	env.AddTest("escaped", tests.TestSafe) // alias
	env.AddTest("sameas", tests.TestSameAs)
	env.AddTest("lower", tests.TestLower)
	env.AddTest("upper", tests.TestUpper)
	env.AddTest("filter", tests.TestFilter)
	env.AddTest("test", tests.TestTest)
}

func registerDefaultFunctions(env *Environment) {
	env.AddFunction("range", fnRange)
	env.AddFunction("dict", fnDict)
	env.AddFunction("cycler", fnCycler)
	env.AddFunction("joiner", fnJoiner)
	env.AddFunction("namespace", fnNamespace)
	env.AddFunction("debug", fnDebug)
	env.AddFunction("lipsum", fnLipsum)
}

// --- Functions ---

func fnRange(state *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	var start, stop, step int64 = 0, 0, 1

	// Like in Rust, keyword arguments are passed as a trailing map, which
	// then fails to convert to an integer or is one argument too many.
	if len(kwargs) > 0 {
		args = append(args[:len(args):len(args)], value.FromMap(kwargs))
	}
	if len(args) == 0 {
		return value.Undefined(), NewError(ErrMissingArgument, "missing argument")
	}
	if len(args) > 3 {
		return value.Undefined(), NewError(ErrTooManyArguments, "too many arguments")
	}
	// Like in Rust, the arguments are converted like isize arguments, and
	// `none` for the upper bound or step means that it was not given.
	lower, err := intArg(args[0], "isize")
	if err != nil {
		return value.Undefined(), err
	}
	stop = lower
	if len(args) > 1 && !args[1].IsNone() && !args[1].IsUndefined() {
		start = lower
		if stop, err = intArg(args[1], "isize"); err != nil {
			return value.Undefined(), err
		}
	}
	if len(args) > 2 && !args[2].IsNone() && !args[2].IsUndefined() {
		if step, err = intArg(args[2], "isize"); err != nil {
			return value.Undefined(), err
		}
	}

	if step == 0 {
		return value.Undefined(), NewError(ErrInvalidOperation, "cannot create range with step of 0")
	}

	// the distance between the bounds and the negated step can exceed the
	// int64 range, so the length is computed with unsigned values.
	var length uint64
	if step > 0 && stop > start {
		length = (uint64(stop)-uint64(start)-1)/uint64(step) + 1
	} else if step < 0 && stop < start {
		length = (uint64(start)-uint64(stop)-1)/(-uint64(step)) + 1
	}
	if length > 100000 {
		return value.Undefined(), NewError(ErrInvalidOperation, "range has too many elements")
	}
	// unlike in Rust, ranges are materialized, so check that the result can
	// be paid for before creating it.
	if err := state.ensureSlots(int(length)); err != nil {
		return value.Undefined(), err
	}

	// counting the items keeps the last step from overflowing the value.
	result := make([]value.Value, 0, length)
	for i := uint64(0); i < length; i++ {
		result = append(result, value.FromInt(start+int64(i)*step))
	}
	return value.FromIterator(value.NewIterator("range", result)), nil
}

func fnDict(state *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	result := value.NewMap()

	// First, copy from first positional argument if it's a map
	if len(args) > 0 {
		if m, ok := args[0].AsValueMap(); ok {
			for k, v := range m.All() {
				result.Set(k, v)
			}
		} else {
			// Try to iterate as items
			items, err := state.iterItems(args[0])
			if err != nil {
				return value.Undefined(), err
			}
			if items != nil {
				for _, item := range items {
					if pair, ok := item.AsSlice(); ok && len(pair) == 2 {
						result.Set(pair[0], pair[1])
					}
				}
			}
		}
	}

	// Then apply kwargs (overwriting).  Go does not retain the order of
	// keyword arguments, so they are added in sorted order.
	keys := make([]string, 0, len(kwargs))
	for k := range kwargs {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		result.Set(value.FromString(k), kwargs[k])
	}
	return value.FromValueMap(result), nil
}

func fnCycler(_ *State, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	return value.FromObject(&cyclerObject{
		items: args,
		index: 0,
	}), nil
}

// cyclerObject implements a cycler that cycles through values
type cyclerObject struct {
	items []value.Value
	index int
}

func (c *cyclerObject) GetAttr(name string) value.Value {
	switch name {
	case "next":
		return value.FromCallable(&cyclerNextCallable{cycler: c})
	case "current":
		if len(c.items) == 0 {
			return value.Undefined()
		}
		idx := c.index
		if idx == 0 {
			idx = len(c.items)
		}
		return c.items[idx-1]
	case "reset":
		return value.FromCallable(&cyclerResetCallable{cycler: c})
	}
	return value.Undefined()
}

type cyclerNextCallable struct {
	cycler *cyclerObject
}

func (c *cyclerNextCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
	if len(c.cycler.items) == 0 {
		return value.Undefined(), nil
	}
	result := c.cycler.items[c.cycler.index]
	c.cycler.index = (c.cycler.index + 1) % len(c.cycler.items)
	return result, nil
}

type cyclerResetCallable struct {
	cycler *cyclerObject
}

func (c *cyclerResetCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
	c.cycler.index = 0
	return value.Undefined(), nil
}

func fnJoiner(_ *State, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	sep := ", "
	if len(args) > 0 {
		if s, ok := args[0].AsString(); ok {
			sep = s
		}
	}
	return value.FromCallable(&joinerCallable{
		sep:   sep,
		first: true,
	}), nil
}

// joinerCallable implements a joiner that returns separator after first call
type joinerCallable struct {
	sep   string
	first bool
}

func (j *joinerCallable) Call(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	_ = state
	if j.first {
		j.first = false
		return value.FromString(""), nil
	}
	return value.FromString(j.sep), nil
}

func fnNamespace(_ *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ns := &namespaceValue{
		data: make(map[string]value.Value),
	}

	// If first argument is a map, copy from it
	if len(args) > 0 {
		if m, ok := args[0].AsMap(); ok {
			for k, v := range m {
				ns.data[k] = v
			}
		} else if !args[0].IsUndefined() && !args[0].IsNone() {
			return value.Undefined(), NewError(ErrInvalidOperation, "namespace expects a mapping")
		}
	}

	// Apply kwargs
	for k, v := range kwargs {
		ns.data[k] = v
	}
	return value.FromObject(ns), nil
}

// namespaceValue is a mutable namespace object
type namespaceValue struct {
	data map[string]value.Value
}

func (n *namespaceValue) GetAttr(name string) value.Value {
	if v, ok := n.data[name]; ok {
		return v
	}
	return value.Undefined()
}

// ObjectRepr makes namespaces maps like in Rust.
func (n *namespaceValue) ObjectRepr() value.ObjectRepr {
	return value.ObjectReprMap
}

// Keys returns the sorted keys of the namespace like in Rust.
func (n *namespaceValue) Keys() []string {
	keys := make([]string, 0, len(n.data))
	for k := range n.data {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func (n *namespaceValue) SetAttr(name string, val value.Value) {
	n.data[name] = val
}

func (n *namespaceValue) String() string {
	return value.FromMap(n.data).String()
}

func (n *namespaceValue) Map() map[string]value.Value {
	return n.data
}

func fnDebug(state *State, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	// If arguments provided, debug those values
	if len(args) > 0 {
		var parts []string
		for _, arg := range args {
			parts = append(parts, arg.Repr())
		}
		return value.FromString(strings.Join(parts, ", ")), nil
	}

	// Otherwise debug the current state
	var parts []string
	parts = append(parts, fmt.Sprintf("State {"))
	parts = append(parts, fmt.Sprintf("  name: %q,", state.name))
	parts = append(parts, "  current variables: {")

	// Collect variables from scopes
	for i := len(state.scopes) - 1; i >= 0; i-- {
		keys := make([]string, 0, len(state.scopes[i]))
		for k := range state.scopes[i] {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			parts = append(parts, fmt.Sprintf("    %s: %s,", k, state.scopes[i][k].Repr()))
		}
	}
	if !state.rootContext.IsUndefined() {
		if obj, ok := state.rootContext.AsObject(); ok {
			if m, ok := obj.(value.MapObject); ok {
				keys := m.Keys()
				sort.Strings(keys)
				for _, k := range keys {
					parts = append(parts, fmt.Sprintf("    %s: %s,", k, state.rootContext.GetAttr(k).Repr()))
				}
			}
		}
	}
	parts = append(parts, "  }")
	parts = append(parts, "}")

	return value.FromString(strings.Join(parts, "\n")), nil
}

func fnLipsum(_ *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	n := int64(5)
	if len(args) > 0 {
		if nn, ok := args[0].AsInt(); ok {
			n = nn
		}
	}
	if nn, ok := kwargs["n"]; ok {
		if nnn, ok := nn.AsInt(); ok {
			n = nnn
		}
	}

	lorem := "Lorem ipsum dolor sit amet, consectetur adipiscing elit. " +
		"Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. " +
		"Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris. "

	var result strings.Builder
	for i := int64(0); i < n; i++ {
		if i > 0 {
			result.WriteString("\n\n")
		}
		result.WriteString(lorem)
	}
	return value.FromSafeString("<p>" + result.String() + "</p>"), nil
}
