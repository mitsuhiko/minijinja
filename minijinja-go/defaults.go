package minijinja

import (
	"encoding/json"
	"fmt"
	"net/url"
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

// --- Filters ---

func filterUpper(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		return value.FromString(strings.ToUpper(s)), nil
	}
	return val, nil
}

func filterLower(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		return value.FromString(strings.ToLower(s)), nil
	}
	return val, nil
}

func filterCapitalize(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		if len(s) == 0 {
			return val, nil
		}
		runes := []rune(strings.ToLower(s))
		runes[0] = []rune(strings.ToUpper(string(runes[0])))[0]
		return value.FromString(string(runes)), nil
	}
	return val, nil
}

func filterTitle(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		return value.FromString(strings.Title(s)), nil
	}
	return val, nil
}

func filterTrim(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		chars := " \t\n\r"
		if len(args) > 0 {
			if c, ok := args[0].AsString(); ok {
				chars = c
			}
		}
		return value.FromString(strings.Trim(s, chars)), nil
	}
	return val, nil
}

func filterReplace(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		if len(args) < 2 {
			return val, fmt.Errorf("replace requires old and new arguments")
		}
		old, _ := args[0].AsString()
		new, _ := args[1].AsString()
		count := -1
		if len(args) > 2 {
			if c, ok := args[2].AsInt(); ok {
				count = int(c)
			}
		}
		return value.FromString(strings.Replace(s, old, new, count)), nil
	}
	return val, nil
}

func filterDefault(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() || val.IsNone() {
		if len(args) > 0 {
			return args[0], nil
		}
		if def, ok := kwargs["default"]; ok {
			return def, nil
		}
		return value.FromString(""), nil
	}

	// Check boolean flag for empty check
	checkBool := false
	if len(args) > 1 {
		if b, ok := args[1].AsBool(); ok {
			checkBool = b
		}
	}
	if b, ok := kwargs["boolean"]; ok {
		if bb, ok := b.AsBool(); ok {
			checkBool = bb
		}
	}

	if checkBool && !val.IsTrue() {
		if len(args) > 0 {
			return args[0], nil
		}
		return value.FromString(""), nil
	}

	return val, nil
}

func filterSafe(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		return value.FromSafeString(s), nil
	}
	return value.FromSafeString(val.String()), nil
}

func filterEscape(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsSafe() {
		return val, nil
	}
	if s, ok := val.AsString(); ok {
		return value.FromSafeString(EscapeHTML(s)), nil
	}
	return value.FromSafeString(EscapeHTML(val.String())), nil
}

func filterLength(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if l, ok := val.Len(); ok {
		return value.FromInt(int64(l)), nil
	}
	return value.FromInt(0), nil
}

func filterFirst(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if len(items) > 0 {
		return items[0], nil
	}
	return value.Undefined(), nil
}

func filterLast(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if len(items) > 0 {
		return items[len(items)-1], nil
	}
	return value.Undefined(), nil
}

func filterReverse(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		runes := []rune(s)
		for i, j := 0, len(runes)-1; i < j; i, j = i+1, j-1 {
			runes[i], runes[j] = runes[j], runes[i]
		}
		return value.FromString(string(runes)), nil
	}

	items := val.Iter()
	if items != nil {
		result := make([]value.Value, len(items))
		for i, item := range items {
			result[len(items)-1-i] = item
		}
		// Return an iterator, not a sequence - this matches Rust MiniJinja behavior
		return value.FromIterator(value.NewIterator("reversed", result)), nil
	}
	return val, nil
}

func filterSort(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	reverse := false
	if len(args) > 0 {
		if b, ok := args[0].AsBool(); ok {
			reverse = b
		}
	}
	if r, ok := kwargs["reverse"]; ok {
		if b, ok := r.AsBool(); ok {
			reverse = b
		}
	}

	result := make([]value.Value, len(items))
	copy(result, items)

	sort.Slice(result, func(i, j int) bool {
		cmp, ok := result[i].Compare(result[j])
		if !ok {
			return false
		}
		if reverse {
			return cmp > 0
		}
		return cmp < 0
	})

	return value.FromSlice(result), nil
}

func filterJoin(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	sep := ""
	if len(args) > 0 {
		sep, _ = args[0].AsString()
	}

	parts := make([]string, len(items))
	for i, item := range items {
		parts[i] = item.String()
	}
	return value.FromString(strings.Join(parts, sep)), nil
}

func filterList(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items != nil {
		return value.FromSlice(items), nil
	}
	return value.FromSlice(nil), nil
}

func filterUnique(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	seen := make(map[string]bool)
	var result []value.Value
	for _, item := range items {
		key := item.Repr()
		if !seen[key] {
			seen[key] = true
			result = append(result, item)
		}
	}
	return value.FromSlice(result), nil
}

func filterMin(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil || len(items) == 0 {
		return value.Undefined(), nil
	}

	minVal := items[0]
	for _, item := range items[1:] {
		if cmp, ok := item.Compare(minVal); ok && cmp < 0 {
			minVal = item
		}
	}
	return minVal, nil
}

func filterMax(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil || len(items) == 0 {
		return value.Undefined(), nil
	}

	maxVal := items[0]
	for _, item := range items[1:] {
		if cmp, ok := item.Compare(maxVal); ok && cmp > 0 {
			maxVal = item
		}
	}
	return maxVal, nil
}

func filterSum(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return value.FromInt(0), nil
	}

	start := value.FromInt(0)
	if len(args) > 0 {
		start = args[0]
	}

	result := start
	for _, item := range items {
		var err error
		result, err = result.Add(item)
		if err != nil {
			return value.Undefined(), err
		}
	}
	return result, nil
}

func filterBatch(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	lineCount := 1
	if len(args) > 0 {
		if c, ok := args[0].AsInt(); ok && c > 0 {
			lineCount = int(c)
		}
	}

	fillWith := value.Undefined()
	if len(args) > 1 {
		fillWith = args[1]
	}
	if f, ok := kwargs["fill_with"]; ok {
		fillWith = f
	}

	var result []value.Value
	for i := 0; i < len(items); i += lineCount {
		end := i + lineCount
		if end > len(items) {
			end = len(items)
		}
		batch := make([]value.Value, end-i)
		copy(batch, items[i:end])

		// Fill the last batch if needed
		if !fillWith.IsUndefined() && len(batch) < lineCount {
			for len(batch) < lineCount {
				batch = append(batch, fillWith)
			}
		}
		result = append(result, value.FromSlice(batch))
	}
	return value.FromSlice(result), nil
}

func filterSlice(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	sliceCount := 1
	if len(args) > 0 {
		if c, ok := args[0].AsInt(); ok && c > 0 {
			sliceCount = int(c)
		}
	}

	fillWith := value.Undefined()
	if len(args) > 1 {
		fillWith = args[1]
	}

	// Calculate slice sizes
	length := len(items)
	baseSize := length / sliceCount
	remainder := length % sliceCount

	var result []value.Value
	offset := 0
	for i := 0; i < sliceCount; i++ {
		size := baseSize
		if i < remainder {
			size++
		}

		slice := make([]value.Value, size)
		copy(slice, items[offset:offset+size])

		// Fill if needed for last slice
		if !fillWith.IsUndefined() && i == sliceCount-1 && size < baseSize+1 {
			for len(slice) < baseSize+1 {
				slice = append(slice, fillWith)
			}
		}

		result = append(result, value.FromSlice(slice))
		offset += size
	}
	return value.FromSlice(result), nil
}

func filterMap(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	// Get attribute name
	var attrName string
	if attr, ok := kwargs["attribute"]; ok {
		if s, ok := attr.AsString(); ok {
			attrName = s
		}
	}

	if attrName == "" {
		return val, fmt.Errorf("map filter requires 'attribute' argument")
	}

	var result []value.Value
	for _, item := range items {
		result = append(result, item.GetAttr(attrName))
	}
	return value.FromSlice(result), nil
}

func filterSelect(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	var result []value.Value
	for _, item := range items {
		if item.IsTrue() {
			result = append(result, item)
		}
	}
	return value.FromSlice(result), nil
}

func filterReject(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	var result []value.Value
	for _, item := range items {
		if !item.IsTrue() {
			result = append(result, item)
		}
	}
	return value.FromSlice(result), nil
}

func filterSelectAttr(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	if len(args) < 1 {
		return val, fmt.Errorf("selectattr requires attribute name")
	}
	attrName, _ := args[0].AsString()

	var result []value.Value
	for _, item := range items {
		attr := item.GetAttr(attrName)
		if attr.IsTrue() {
			result = append(result, item)
		}
	}
	return value.FromSlice(result), nil
}

func filterRejectAttr(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	if len(args) < 1 {
		return val, fmt.Errorf("rejectattr requires attribute name")
	}
	attrName, _ := args[0].AsString()

	var result []value.Value
	for _, item := range items {
		attr := item.GetAttr(attrName)
		if !attr.IsTrue() {
			result = append(result, item)
		}
	}
	return value.FromSlice(result), nil
}

func filterAbs(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if i, ok := val.AsInt(); ok {
		if i < 0 {
			return value.FromInt(-i), nil
		}
		return val, nil
	}
	if f, ok := val.AsFloat(); ok {
		if f < 0 {
			return value.FromFloat(-f), nil
		}
		return val, nil
	}
	return val, nil
}

func filterInt(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	defaultVal := value.FromInt(0)
	if len(args) > 0 {
		defaultVal = args[0]
	}
	if d, ok := kwargs["default"]; ok {
		defaultVal = d
	}

	if i, ok := val.AsInt(); ok {
		return value.FromInt(i), nil
	}
	if f, ok := val.AsFloat(); ok {
		return value.FromInt(int64(f)), nil
	}
	if s, ok := val.AsString(); ok {
		s = strings.TrimSpace(s)
		var i int64
		if _, err := fmt.Sscanf(s, "%d", &i); err == nil {
			return value.FromInt(i), nil
		}
	}
	return defaultVal, nil
}

func filterFloat(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	defaultVal := value.FromFloat(0.0)
	if len(args) > 0 {
		defaultVal = args[0]
	}
	if d, ok := kwargs["default"]; ok {
		defaultVal = d
	}

	if f, ok := val.AsFloat(); ok {
		return value.FromFloat(f), nil
	}
	if s, ok := val.AsString(); ok {
		s = strings.TrimSpace(s)
		var f float64
		if _, err := fmt.Sscanf(s, "%f", &f); err == nil {
			return value.FromFloat(f), nil
		}
	}
	return defaultVal, nil
}

func filterRound(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	f, ok := val.AsFloat()
	if !ok {
		return val, nil
	}

	precision := 0
	if len(args) > 0 {
		if p, ok := args[0].AsInt(); ok {
			precision = int(p)
		}
	}
	if p, ok := kwargs["precision"]; ok {
		if pp, ok := p.AsInt(); ok {
			precision = int(pp)
		}
	}

	method := "common"
	if len(args) > 1 {
		if m, ok := args[1].AsString(); ok {
			method = m
		}
	}
	if m, ok := kwargs["method"]; ok {
		if mm, ok := m.AsString(); ok {
			method = mm
		}
	}

	multiplier := 1.0
	for i := 0; i < precision; i++ {
		multiplier *= 10
	}

	switch method {
	case "floor":
		f = float64(int64(f*multiplier)) / multiplier
	case "ceil":
		f = float64(int64(f*multiplier+0.9999999999)) / multiplier
	default: // common
		f = float64(int64(f*multiplier+0.5)) / multiplier
	}

	if precision == 0 {
		return value.FromInt(int64(f)), nil
	}
	return value.FromFloat(f), nil
}

func filterItems(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if m, ok := val.AsMap(); ok {
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)

		var result []value.Value
		for _, k := range keys {
			result = append(result, value.FromSlice([]value.Value{
				value.FromString(k),
				m[k],
			}))
		}
		return value.FromSlice(result), nil
	}
	return value.FromSlice(nil), nil
}

func filterKeys(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if m, ok := val.AsMap(); ok {
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)

		result := make([]value.Value, len(keys))
		for i, k := range keys {
			result[i] = value.FromString(k)
		}
		return value.FromSlice(result), nil
	}
	return value.FromSlice(nil), nil
}

func filterValues(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if m, ok := val.AsMap(); ok {
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)

		result := make([]value.Value, len(keys))
		for i, k := range keys {
			result[i] = m[k]
		}
		return value.FromSlice(result), nil
	}
	return value.FromSlice(nil), nil
}

func filterDictSort(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if m, ok := val.AsMap(); ok {
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}

		byValue := false
		if len(args) > 1 {
			if b, ok := args[1].AsBool(); ok {
				byValue = b
			}
		}
		if b, ok := kwargs["by"]; ok {
			if s, ok := b.AsString(); ok && s == "value" {
				byValue = true
			}
		}

		reverse := false
		if len(args) > 2 {
			if b, ok := args[2].AsBool(); ok {
				reverse = b
			}
		}
		if b, ok := kwargs["reverse"]; ok {
			if bb, ok := b.AsBool(); ok {
				reverse = bb
			}
		}

		if byValue {
			sort.Slice(keys, func(i, j int) bool {
				cmp, ok := m[keys[i]].Compare(m[keys[j]])
				if !ok {
					return keys[i] < keys[j]
				}
				if reverse {
					return cmp > 0
				}
				return cmp < 0
			})
		} else {
			sort.Slice(keys, func(i, j int) bool {
				if reverse {
					return keys[i] > keys[j]
				}
				return keys[i] < keys[j]
			})
		}

		var result []value.Value
		for _, k := range keys {
			result = append(result, value.FromSlice([]value.Value{
				value.FromString(k),
				m[k],
			}))
		}
		return value.FromSlice(result), nil
	}
	return value.FromSlice(nil), nil
}

func filterAttr(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	if len(args) < 1 {
		return value.Undefined(), fmt.Errorf("attr filter requires attribute name")
	}
	name, _ := args[0].AsString()
	return val.GetAttr(name), nil
}

func filterIndent(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	s, ok := val.AsString()
	if !ok {
		return val, nil
	}

	width := 4
	if len(args) > 0 {
		if w, ok := args[0].AsInt(); ok {
			width = int(w)
		}
	}
	if w, ok := kwargs["width"]; ok {
		if ww, ok := w.AsInt(); ok {
			width = int(ww)
		}
	}

	first := false
	if len(args) > 1 {
		if b, ok := args[1].AsBool(); ok {
			first = b
		}
	}
	if f, ok := kwargs["first"]; ok {
		if ff, ok := f.AsBool(); ok {
			first = ff
		}
	}

	blank := false
	if len(args) > 2 {
		if b, ok := args[2].AsBool(); ok {
			blank = b
		}
	}
	if b, ok := kwargs["blank"]; ok {
		if bb, ok := b.AsBool(); ok {
			blank = bb
		}
	}

	indent := strings.Repeat(" ", width)
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		if i == 0 && !first {
			continue
		}
		if line == "" && !blank {
			continue
		}
		lines[i] = indent + line
	}
	return value.FromString(strings.Join(lines, "\n")), nil
}

func filterPprint(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	return value.FromString(val.Repr()), nil
}

func filterTojson(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	// Convert value to Go native type for JSON serialization
	native := valueToNative(val)

	// Check for indent option
	// If first arg is bool: true = indent 2, false = no indent
	// If first arg is int: that number of spaces
	indent := ""
	if len(args) > 0 {
		if b, ok := args[0].AsBool(); ok {
			if b {
				indent = "  " // 2 spaces for true
			}
		} else if i, ok := args[0].AsInt(); ok {
			indent = strings.Repeat(" ", int(i))
		}
	}
	if i, ok := kwargs["indent"]; ok {
		if b, ok := i.AsBool(); ok {
			if b {
				indent = "  "
			}
		} else if n, ok := i.AsInt(); ok {
			indent = strings.Repeat(" ", int(n))
		}
	}

	var data []byte
	var err error
	if indent != "" {
		data, err = json.MarshalIndent(native, "", indent)
	} else {
		data, err = json.Marshal(native)
	}
	if err != nil {
		return value.Undefined(), err
	}
	return value.FromSafeString(string(data)), nil
}

func valueToNative(v value.Value) interface{} {
	switch v.Kind() {
	case value.KindUndefined, value.KindNone:
		return nil
	case value.KindBool:
		b, _ := v.AsBool()
		return b
	case value.KindNumber:
		if i, ok := v.AsInt(); ok {
			return i
		}
		f, _ := v.AsFloat()
		return f
	case value.KindString:
		s, _ := v.AsString()
		return s
	case value.KindSeq:
		items, _ := v.AsSlice()
		result := make([]interface{}, len(items))
		for i, item := range items {
			result[i] = valueToNative(item)
		}
		return result
	case value.KindMap:
		m, _ := v.AsMap()
		result := make(map[string]interface{})
		for k, val := range m {
			result[k] = valueToNative(val)
		}
		return result
	default:
		return v.String()
	}
}

func filterUrlencode(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, ok := val.AsString()
	if !ok {
		s = val.String()
	}
	return value.FromString(url.QueryEscape(s)), nil
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

func testOdd(_ *State, val value.Value, _ []value.Value) (bool, error) {
	if i, ok := val.AsInt(); ok {
		return i%2 != 0, nil
	}
	return false, nil
}

func testEven(_ *State, val value.Value, _ []value.Value) (bool, error) {
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
	// An integer is a number that is stored as int64 (not float64)
	_, ok := val.AsInt()
	if !ok {
		return false, nil
	}
	// Also ensure it's not stored as a float
	return val.IsActualInt(), nil
}

func testFloat(_ *State, val value.Value, _ []value.Value) (bool, error) {
	// A float is a number stored as float64
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
	// sameas checks for identity, not equality
	// For primitive types this is the same as equality with strict type checking
	return val.SameAs(args[0]), nil
}

func testLower(_ *State, val value.Value, _ []value.Value) (bool, error) {
	s, ok := val.AsString()
	if !ok {
		return false, nil
	}
	// Check if all characters are lowercase (like Rust's is_lowercase)
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
	// Check if all characters are uppercase (like Rust's is_uppercase)
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
	// Return an iterator, not a sequence - this matches Rust MiniJinja behavior
	return value.FromIterator(value.NewIterator("range", result)), nil
}

func fnDict(_ *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	result := make(map[string]value.Value)
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

func fnNamespace(_ *State, _ []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ns := &namespaceValue{
		data: make(map[string]value.Value),
	}
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

func fnDebug(state *State, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	var parts []string
	parts = append(parts, fmt.Sprintf("Template: %s", state.name))
	parts = append(parts, "Variables:")
	for i := len(state.scopes) - 1; i >= 0; i-- {
		for k, v := range state.scopes[i] {
			parts = append(parts, fmt.Sprintf("  %s = %s", k, v.Repr()))
		}
	}
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
