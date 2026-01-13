package minijinja

import (
	"fmt"
	"sort"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/value"
)

func registerDefaultFilters(env *Environment) {
	// String filters
	env.AddFilter("upper", FilterUpper)
	env.AddFilter("lower", FilterLower)
	env.AddFilter("capitalize", FilterCapitalize)
	env.AddFilter("title", FilterTitle)
	env.AddFilter("trim", FilterTrim)
	env.AddFilter("replace", FilterReplace)
	env.AddFilter("default", FilterDefault)
	env.AddFilter("d", FilterDefault) // alias
	env.AddFilter("safe", FilterSafe)
	env.AddFilter("escape", FilterEscape)
	env.AddFilter("e", FilterEscape) // alias
	env.AddFilter("string", FilterString)
	env.AddFilter("bool", FilterBool)
	env.AddFilter("split", FilterSplit)
	env.AddFilter("lines", FilterLines)

	// List/sequence filters
	env.AddFilter("length", FilterLength)
	env.AddFilter("count", FilterLength) // alias
	env.AddFilter("first", FilterFirst)
	env.AddFilter("last", FilterLast)
	env.AddFilter("reverse", FilterReverse)
	env.AddFilter("sort", FilterSort)
	env.AddFilter("join", FilterJoin)
	env.AddFilter("list", FilterList)
	env.AddFilter("unique", FilterUnique)
	env.AddFilter("min", FilterMin)
	env.AddFilter("max", FilterMax)
	env.AddFilter("sum", FilterSum)
	env.AddFilter("batch", FilterBatch)
	env.AddFilter("slice", FilterSlice)
	env.AddFilter("map", FilterMap)
	env.AddFilter("select", FilterSelect)
	env.AddFilter("reject", FilterReject)
	env.AddFilter("selectattr", FilterSelectAttr)
	env.AddFilter("rejectattr", FilterRejectAttr)
	env.AddFilter("groupby", FilterGroupBy)
	env.AddFilter("chain", FilterChain)
	env.AddFilter("zip", FilterZip)

	// Numeric filters
	env.AddFilter("abs", FilterAbs)
	env.AddFilter("int", FilterInt)
	env.AddFilter("float", FilterFloat)
	env.AddFilter("round", FilterRound)

	// Dict filters
	env.AddFilter("items", FilterItems)
	env.AddFilter("keys", FilterKeys)
	env.AddFilter("values", FilterValues)
	env.AddFilter("dictsort", FilterDictSort)

	// Other filters
	env.AddFilter("attr", FilterAttr)
	env.AddFilter("indent", FilterIndent)
	env.AddFilter("pprint", FilterPprint)

	// JSON and URL filters
	env.AddFilter("tojson", FilterTojson)
	env.AddFilter("urlencode", FilterUrlencode)
}

func registerDefaultTests(env *Environment) {
	env.AddTest("defined", TestDefined)
	env.AddTest("undefined", TestUndefined)
	env.AddTest("none", TestNone)
	env.AddTest("true", TestTrue)
	env.AddTest("false", TestFalse)
	env.AddTest("odd", TestOdd)
	env.AddTest("even", TestEven)
	env.AddTest("divisibleby", TestDivisibleBy)
	env.AddTest("eq", TestEq)
	env.AddTest("equalto", TestEq)
	env.AddTest("ne", TestNe)
	env.AddTest("lt", TestLt)
	env.AddTest("le", TestLe)
	env.AddTest("gt", TestGt)
	env.AddTest("ge", TestGe)
	env.AddTest("in", TestIn)
	env.AddTest("string", TestString)
	env.AddTest("number", TestNumber)
	env.AddTest("integer", TestInteger)
	env.AddTest("int", TestInteger) // alias
	env.AddTest("float", TestFloat)
	env.AddTest("boolean", TestBoolean)
	env.AddTest("sequence", TestSequence)
	env.AddTest("mapping", TestMapping)
	env.AddTest("iterable", TestIterable)
	env.AddTest("startingwith", TestStartingWith)
	env.AddTest("endingwith", TestEndingWith)
	env.AddTest("containing", TestContaining)
	env.AddTest("safe", TestSafe)
	env.AddTest("escaped", TestSafe) // alias
	env.AddTest("sameas", TestSameAs)
	env.AddTest("lower", TestLower)
	env.AddTest("upper", TestUpper)
	env.AddTest("filter", TestFilter)
	env.AddTest("test", TestTest)
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
