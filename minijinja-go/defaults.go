package minijinja

import (
	"fmt"
	"sort"
	"strings"
	"unicode"

	"github.com/mitsuhiko/minijinja/minijinja-go/value"
)

func registerDefaultFilters(env *Environment) {
	// String filters
	env.AddFilter("upper", filterUpper)
	env.AddFilter("lower", filterLower)
	env.AddFilter("capitalize", filterCapitalize)
	env.AddFilter("title", filterTitle)
	env.AddFilter("trim", filterTrim)
	env.AddFilter("replace", filterReplace)
	env.AddFilter("default", filterDefault)
	env.AddFilter("d", filterDefault) // alias
	env.AddFilter("safe", filterSafe)
	env.AddFilter("escape", filterEscape)
	env.AddFilter("e", filterEscape) // alias
	env.AddFilter("string", filterString)
	env.AddFilter("bool", filterBool)
	env.AddFilter("split", filterSplit)
	env.AddFilter("lines", filterLines)

	// List/sequence filters
	env.AddFilter("length", filterLength)
	env.AddFilter("count", filterLength) // alias
	env.AddFilter("first", filterFirst)
	env.AddFilter("last", filterLast)
	env.AddFilter("reverse", filterReverse)
	env.AddFilter("sort", filterSort)
	env.AddFilter("join", filterJoin)
	env.AddFilter("list", filterList)
	env.AddFilter("unique", filterUnique)
	env.AddFilter("min", filterMin)
	env.AddFilter("max", filterMax)
	env.AddFilter("sum", filterSum)
	env.AddFilter("batch", filterBatch)
	env.AddFilter("slice", filterSlice)
	env.AddFilter("map", filterMap)
	env.AddFilter("select", filterSelect)
	env.AddFilter("reject", filterReject)
	env.AddFilter("selectattr", filterSelectAttr)
	env.AddFilter("rejectattr", filterRejectAttr)
	env.AddFilter("groupby", filterGroupBy)
	env.AddFilter("chain", filterChain)
	env.AddFilter("zip", filterZip)

	// Numeric filters
	env.AddFilter("abs", filterAbs)
	env.AddFilter("int", filterInt)
	env.AddFilter("float", filterFloat)
	env.AddFilter("round", filterRound)

	// Dict filters
	env.AddFilter("items", filterItems)
	env.AddFilter("keys", filterKeys)
	env.AddFilter("values", filterValues)
	env.AddFilter("dictsort", filterDictSort)

	// Other filters
	env.AddFilter("attr", filterAttr)
	env.AddFilter("indent", filterIndent)
	env.AddFilter("pprint", filterPprint)

	// JSON and URL filters
	env.AddFilter("tojson", filterTojson)
	env.AddFilter("urlencode", filterUrlencode)
}

func registerDefaultTests(env *Environment) {
	env.AddTest("defined", testDefined)
	env.AddTest("undefined", testUndefined)
	env.AddTest("none", testNone)
	env.AddTest("true", testTrue)
	env.AddTest("false", testFalse)
	env.AddTest("odd", testOdd)
	env.AddTest("even", testEven)
	env.AddTest("divisibleby", testDivisibleBy)
	env.AddTest("eq", testEq)
	env.AddTest("equalto", testEq)
	env.AddTest("ne", testNe)
	env.AddTest("lt", testLt)
	env.AddTest("le", testLe)
	env.AddTest("gt", testGt)
	env.AddTest("ge", testGe)
	env.AddTest("in", testIn)
	env.AddTest("string", testString)
	env.AddTest("number", testNumber)
	env.AddTest("integer", testInteger)
	env.AddTest("int", testInteger) // alias
	env.AddTest("float", testFloat)
	env.AddTest("boolean", testBoolean)
	env.AddTest("sequence", testSequence)
	env.AddTest("mapping", testMapping)
	env.AddTest("iterable", testIterable)
	env.AddTest("startingwith", testStartingWith)
	env.AddTest("endingwith", testEndingWith)
	env.AddTest("containing", testContaining)
	env.AddTest("safe", testSafe)
	env.AddTest("escaped", testSafe) // alias
	env.AddTest("sameas", testSameAs)
	env.AddTest("lower", testLower)
	env.AddTest("upper", testUpper)
	env.AddTest("filter", testFilter)
	env.AddTest("test", testTest)
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


// --- Tests ---

func testDefined(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return !val.IsUndefined(), nil
}

func testUndefined(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsUndefined(), nil
}

func testNone(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsNone(), nil
}

func testTrue(_ *State, val value.Value, _ []value.Value) (bool, error) {
	if b, ok := val.AsBool(); ok {
		return b, nil
	}
	return false, nil
}

func testFalse(_ *State, val value.Value, _ []value.Value) (bool, error) {
	if b, ok := val.AsBool(); ok {
		return !b, nil
	}
	return false, nil
}

func testOdd(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) > 0 {
		return false, NewError(ErrInvalidOperation, "odd test expects no arguments")
	}
	if i, ok := val.AsInt(); ok {
		return i%2 != 0, nil
	}
	return false, nil
}

func testEven(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) > 0 {
		return false, NewError(ErrInvalidOperation, "even test expects no arguments")
	}
	if i, ok := val.AsInt(); ok {
		return i%2 == 0, nil
	}
	return false, nil
}

func testDivisibleBy(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, fmt.Errorf("divisibleby test requires argument")
	}
	if i, ok := val.AsInt(); ok {
		if d, ok := args[0].AsInt(); ok && d != 0 {
			return i%d == 0, nil
		}
	}
	return false, nil
}

func testEq(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.Equal(args[0]), nil
}

func testNe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return !val.Equal(args[0]), nil
}

func testLt(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp < 0, nil
	}
	return false, nil
}

func testLe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp <= 0, nil
	}
	return false, nil
}

func testGt(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp > 0, nil
	}
	return false, nil
}

func testGe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp >= 0, nil
	}
	return false, nil
}

func testIn(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return args[0].Contains(val), nil
}

func testString(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindString, nil
}

func testNumber(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindNumber, nil
}

func testInteger(_ *State, val value.Value, _ []value.Value) (bool, error) {
	_, ok := val.AsInt()
	if !ok {
		return false, nil
	}
	return val.IsActualInt(), nil
}

func testFloat(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsActualFloat(), nil
}

func testBoolean(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindBool, nil
}

func testSafe(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsSafe(), nil
}

func testSameAs(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.SameAs(args[0]), nil
}

func testLower(_ *State, val value.Value, _ []value.Value) (bool, error) {
	s, ok := val.AsString()
	if !ok {
		return false, nil
	}
	for _, r := range s {
		if !unicode.IsLower(r) && unicode.IsLetter(r) {
			return false, nil
		}
	}
	return true, nil
}

func testUpper(_ *State, val value.Value, _ []value.Value) (bool, error) {
	s, ok := val.AsString()
	if !ok {
		return false, nil
	}
	for _, r := range s {
		if !unicode.IsUpper(r) && unicode.IsLetter(r) {
			return false, nil
		}
	}
	return true, nil
}

func testFilter(state *State, val value.Value, _ []value.Value) (bool, error) {
	name, ok := val.AsString()
	if !ok {
		return false, nil
	}
	_, exists := state.env.getFilter(name)
	return exists, nil
}

func testTest(state *State, val value.Value, _ []value.Value) (bool, error) {
	name, ok := val.AsString()
	if !ok {
		return false, nil
	}
	_, exists := state.env.getTest(name)
	return exists, nil
}

func testSequence(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindSeq, nil
}

func testMapping(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindMap, nil
}

func testIterable(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Iter() != nil, nil
}

func testStartingWith(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if s, ok := val.AsString(); ok {
		if prefix, ok := args[0].AsString(); ok {
			return strings.HasPrefix(s, prefix), nil
		}
	}
	return false, nil
}

func testEndingWith(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if s, ok := val.AsString(); ok {
		if suffix, ok := args[0].AsString(); ok {
			return strings.HasSuffix(s, suffix), nil
		}
	}
	return false, nil
}

func testContaining(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.Contains(args[0]), nil
}

// --- Functions ---

func fnRange(_ *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	var start, stop, step int64 = 0, 0, 1

	switch len(args) {
	case 1:
		stop, _ = args[0].AsInt()
	case 2:
		start, _ = args[0].AsInt()
		stop, _ = args[1].AsInt()
	case 3:
		start, _ = args[0].AsInt()
		stop, _ = args[1].AsInt()
		step, _ = args[2].AsInt()
	default:
		return value.FromIterator(value.NewIterator("range", nil)), nil
	}

	if step == 0 {
		return value.Undefined(), fmt.Errorf("range step cannot be zero")
	}

	length := int64(0)
	if step > 0 {
		if stop > start {
			length = (stop-start + step - 1) / step
		}
	} else {
		if stop < start {
			negStep := -step
			length = (start-stop + negStep - 1) / negStep
		}
	}
	if length > 100000 {
		return value.Undefined(), NewError(ErrInvalidOperation, "range has too many elements")
	}

	var result []value.Value
	if step > 0 {
		for i := start; i < stop; i += step {
			result = append(result, value.FromInt(i))
		}
	} else {
		for i := start; i > stop; i += step {
			result = append(result, value.FromInt(i))
		}
	}
	return value.FromIterator(value.NewIterator("range", result)), nil
}

func fnDict(_ *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	result := make(map[string]value.Value)

	// First, copy from first positional argument if it's a map
	if len(args) > 0 {
		if m, ok := args[0].AsMap(); ok {
			for k, v := range m {
				result[k] = v
			}
		} else {
			// Try to iterate as items
			items := args[0].Iter()
			if items != nil {
				for _, item := range items {
					if pair, ok := item.AsSlice(); ok && len(pair) == 2 {
						if k, ok := pair[0].AsString(); ok {
							result[k] = pair[1]
						}
					}
				}
			}
		}
	}

	// Then apply kwargs (overwriting)
	for k, v := range kwargs {
		result[k] = v
	}
	return value.FromMap(result), nil
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

func (c *cyclerNextCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

func (c *cyclerResetCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

func (j *joinerCallable) Call(args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

func (n *namespaceValue) SetAttr(name string, val value.Value) {
	n.data[name] = val
}

func (n *namespaceValue) String() string {
	keys := make([]string, 0, len(n.data))
	for k := range n.data {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	parts := make([]string, 0, len(keys))
	for _, k := range keys {
		parts = append(parts, fmt.Sprintf("%q: %s", k, n.data[k].Repr()))
	}
	return "{" + strings.Join(parts, ", ") + "}"
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
