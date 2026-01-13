package minijinja

import (
	"encoding/json"
	"fmt"
	"math"
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
		// Title case: capitalize after whitespace and some punctuation
		var result strings.Builder
		capitalizeNext := true
		for _, r := range s {
			if unicode.IsSpace(r) || r == '-' || r == '_' || r == ':' || r == ',' || r == '.' {
				capitalizeNext = true
				result.WriteRune(r)
			} else if capitalizeNext {
				result.WriteRune(unicode.ToUpper(r))
				capitalizeNext = false
			} else {
				result.WriteRune(unicode.ToLower(r))
			}
		}
		return value.FromString(result.String()), nil
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

func filterString(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.Kind() == value.KindString {
		return val, nil
	}
	return value.FromString(val.String()), nil
}

func filterBool(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	return value.FromBool(val.IsTrue()), nil
}

func filterSplit(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, ok := val.AsString()
	if !ok {
		return value.FromSlice(nil), nil
	}

	// Get split pattern
	var splitOn *string
	if len(args) > 0 && !args[0].IsNone() {
		if sp, ok := args[0].AsString(); ok {
			splitOn = &sp
		}
	}

	// Get max splits
	maxSplits := -1
	if len(args) > 1 {
		if m, ok := args[1].AsInt(); ok {
			maxSplits = int(m) + 1 // Go's SplitN uses count, not number of splits
		}
	}

	var parts []string
	if splitOn == nil {
		// Split on whitespace
		if maxSplits <= 0 {
			parts = strings.Fields(s)
		} else {
			// Custom split with max, keeping empty strings filtered
			parts = splitWhitespaceN(s, maxSplits)
		}
	} else {
		if maxSplits <= 0 {
			parts = strings.Split(s, *splitOn)
		} else {
			parts = strings.SplitN(s, *splitOn, maxSplits)
		}
	}

	result := make([]value.Value, len(parts))
	for i, p := range parts {
		result[i] = value.FromString(p)
	}
	return value.FromIterator(value.NewIterator("split", result)), nil
}

func splitWhitespaceN(s string, n int) []string {
	var result []string
	start := -1
	for i, r := range s {
		if unicode.IsSpace(r) {
			if start >= 0 {
				result = append(result, s[start:i])
				start = -1
				if len(result) >= n-1 {
					// Find next non-space and take rest
					for j := i; j < len(s); j++ {
						if !unicode.IsSpace(rune(s[j])) {
							result = append(result, s[j:])
							return result
						}
					}
					return result
				}
			}
		} else {
			if start < 0 {
				start = i
			}
		}
	}
	if start >= 0 {
		result = append(result, s[start:])
	}
	return result
}

func filterLines(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, ok := val.AsString()
	if !ok {
		return value.FromSlice(nil), nil
	}

	// Normalize line endings and split
	s = strings.ReplaceAll(s, "\r\n", "\n")
	s = strings.ReplaceAll(s, "\r", "\n")
	lines := strings.Split(s, "\n")

	result := make([]value.Value, len(lines))
	for i, line := range lines {
		result[i] = value.FromString(line)
	}
	return value.FromSlice(result), nil
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
		return value.FromIterator(value.NewIterator("reversed", result)), nil
	}
	return val, nil
}

func filterSort(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

	caseSensitive := false
	if cs, ok := kwargs["case_sensitive"]; ok {
		if b, ok := cs.AsBool(); ok {
			caseSensitive = b
		}
	}

	// Get attribute for sorting
	var attrName string
	if attr, ok := kwargs["attribute"]; ok {
		if s, ok := attr.AsString(); ok {
			attrName = s
		}
	}

	result := make([]value.Value, len(items))
	copy(result, items)

	sort.SliceStable(result, func(i, j int) bool {
		a, b := result[i], result[j]

		// Apply attribute if specified
		if attrName != "" {
			a = getDeepAttr(a, attrName)
			b = getDeepAttr(b, attrName)
		}

		// Case-insensitive string comparison
		if !caseSensitive {
			if s1, ok1 := a.AsString(); ok1 {
				if s2, ok2 := b.AsString(); ok2 {
					cmp := strings.Compare(strings.ToLower(s1), strings.ToLower(s2))
					if reverse {
						return cmp > 0
					}
					return cmp < 0
				}
			}
		}

		cmp, ok := a.Compare(b)
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

// getDeepAttr gets a nested attribute (supports "a.b.0" syntax)
func getDeepAttr(v value.Value, path string) value.Value {
	parts := strings.Split(path, ".")
	for _, part := range parts {
		// Try as integer index first
		if idx, err := parseInt(part); err == nil {
			v = v.GetItem(value.FromInt(idx))
		} else {
			v = v.GetAttr(part)
		}
		if v.IsUndefined() {
			return v
		}
	}
	return v
}

func parseInt(s string) (int64, error) {
	var n int64
	_, err := fmt.Sscanf(s, "%d", &n)
	return n, err
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

func filterUnique(_ *State, val value.Value, _ []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	caseSensitive := false
	if cs, ok := kwargs["case_sensitive"]; ok {
		if b, ok := cs.AsBool(); ok {
			caseSensitive = b
		}
	}

	seen := make(map[string]bool)
	var result []value.Value
	for _, item := range items {
		var key string
		if !caseSensitive {
			if s, ok := item.AsString(); ok {
				key = strings.ToLower(s)
			} else {
				key = item.Repr()
			}
		} else {
			key = item.Repr()
		}
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
		return value.Undefined(), NewError(ErrInvalidOperation, "cannot slice non-iterable")
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
	maxSize := baseSize
	if remainder > 0 {
		maxSize++
	}

	var result []value.Value
	offset := 0
	for i := 0; i < sliceCount; i++ {
		size := baseSize
		if i < remainder {
			size++
		}

		slice := make([]value.Value, size)
		copy(slice, items[offset:offset+size])

		// Fill slices to the maximum size when requested
		if !fillWith.IsUndefined() && len(slice) < maxSize {
			for len(slice) < maxSize {
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

	// Check for filter name as first positional arg
	var filterName string
	if len(args) > 0 {
		if s, ok := args[0].AsString(); ok {
			filterName = s
		}
	}

	// Get attribute name
	var attrName string
	attrValue := value.Undefined()
	if attr, ok := kwargs["attribute"]; ok {
		attrValue = attr
		if s, ok := attr.AsString(); ok {
			attrName = s
		}
	}

	// Get default value
	defaultVal := value.Undefined()
	if def, ok := kwargs["default"]; ok {
		defaultVal = def
	}

	var result []value.Value
	for _, item := range items {
		var mapped value.Value
		if !attrValue.IsUndefined() {
			// Attribute mapping with dot notation support
			if attrName != "" {
				mapped = getDeepAttr(item, attrName)
			} else {
				mapped = item.GetItem(attrValue)
			}
			if mapped.IsUndefined() && !defaultVal.IsUndefined() {
				mapped = defaultVal
			}
		} else if filterName != "" {
			// Filter mapping
			filterFn, ok := state.env.getFilter(filterName)
			if !ok {
				return val, fmt.Errorf("unknown filter: %s", filterName)
			}
			var err error
			mapped, err = filterFn(state, item, args[1:], kwargs)
			if err != nil {
				return val, err
			}
		} else {
			return val, fmt.Errorf("map filter requires 'attribute' or filter name argument")
		}
		result = append(result, mapped)
	}
	return value.FromSlice(result), nil
}

func normalizeTestName(name string) string {
	switch name {
	case "==":
		return "eq"
	case "!=":
		return "ne"
	case ">":
		return "gt"
	case ">=":
		return "ge"
	case "<":
		return "lt"
	case "<=":
		return "le"
	default:
		return name
	}
}

func filterSelect(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	// Get test name if provided
	var testName string
	if len(args) > 0 {
		if s, ok := args[0].AsString(); ok {
			testName = normalizeTestName(s)
		}
	}

	var result []value.Value
	for _, item := range items {
		var keep bool
		if testName != "" {
			testFn, ok := state.env.getTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			var err error
			keep, err = testFn(state, item, args[1:])
			if err != nil {
				return val, err
			}
		} else {
			keep = item.IsTrue()
		}
		if keep {
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

	// Get test name if provided
	var testName string
	if len(args) > 0 {
		if s, ok := args[0].AsString(); ok {
			testName = normalizeTestName(s)
		}
	}

	var result []value.Value
	for _, item := range items {
		var reject bool
		if testName != "" {
			testFn, ok := state.env.getTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			var err error
			reject, err = testFn(state, item, args[1:])
			if err != nil {
				return val, err
			}
		} else {
			reject = item.IsTrue()
		}
		if !reject {
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

	// Get test name if provided (second arg)
	var testName string
	if len(args) > 1 {
		if s, ok := args[1].AsString(); ok {
			testName = normalizeTestName(s)
		}
	}

	var result []value.Value
	for _, item := range items {
		attr := item.GetAttr(attrName)
		var keep bool
		if testName != "" {
			testFn, ok := state.env.getTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			var err error
			keep, err = testFn(state, attr, args[2:])
			if err != nil {
				return val, err
			}
		} else {
			keep = attr.IsTrue()
		}
		if keep {
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

	// Get test name if provided (second arg)
	var testName string
	if len(args) > 1 {
		if s, ok := args[1].AsString(); ok {
			testName = normalizeTestName(s)
		}
	}

	var result []value.Value
	for _, item := range items {
		attr := item.GetAttr(attrName)
		var reject bool
		if testName != "" {
			testFn, ok := state.env.getTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			var err error
			reject, err = testFn(state, attr, args[2:])
			if err != nil {
				return val, err
			}
		} else {
			reject = attr.IsTrue()
		}
		if !reject {
			result = append(result, item)
		}
	}
	return value.FromSlice(result), nil
}

func filterGroupBy(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items == nil {
		return val, nil
	}

	// Get attribute name
	var attrName string
	if len(args) > 0 {
		if s, ok := args[0].AsString(); ok {
			attrName = s
		}
	}
	if attr, ok := kwargs["attribute"]; ok {
		if s, ok := attr.AsString(); ok {
			attrName = s
		}
	}
	if attrName == "" {
		return val, fmt.Errorf("groupby requires attribute name")
	}

	// Get default value
	defaultVal := value.Undefined()
	if def, ok := kwargs["default"]; ok {
		defaultVal = def
	}
	if len(args) > 1 && defaultVal.IsUndefined() {
		defaultVal = args[1]
	}

	// Case sensitivity
	caseSensitive := false
	if cs, ok := kwargs["case_sensitive"]; ok {
		if b, ok := cs.AsBool(); ok {
			caseSensitive = b
		}
	}

	// Sort items by group key
	sorted := make([]value.Value, len(items))
	copy(sorted, items)

	sort.SliceStable(sorted, func(i, j int) bool {
		left := groupByValue(sorted[i], attrName, defaultVal)
		right := groupByValue(sorted[j], attrName, defaultVal)
		return compareGroupBy(left, right, caseSensitive) < 0
	})

	// Group items
	var result []value.Value
	var currentGrouper value.Value
	var currentList []value.Value
	hasGroup := false

	for _, item := range sorted {
		groupValue := groupByValue(item, attrName, defaultVal)
		if !hasGroup {
			currentGrouper = groupValue
			currentList = []value.Value{item}
			hasGroup = true
			continue
		}

		if !groupByEqual(currentGrouper, groupValue, caseSensitive) {
			result = append(result, value.FromObject(&groupObject{
				grouper: currentGrouper,
				list:    currentList,
			}))
			currentGrouper = groupValue
			currentList = []value.Value{item}
			continue
		}

		currentGrouper = groupValue
		currentList = append(currentList, item)
	}

	if hasGroup {
		result = append(result, value.FromObject(&groupObject{
			grouper: currentGrouper,
			list:    currentList,
		}))
	}

	return value.FromSlice(result), nil
}

func groupByValue(item value.Value, attrName string, defaultVal value.Value) value.Value {
	grouper := getDeepAttr(item, attrName)
	if grouper.IsUndefined() {
		grouper = defaultVal
	}
	return grouper
}

func compareGroupBy(a, b value.Value, caseSensitive bool) int {
	if !caseSensitive {
		if s1, ok := a.AsString(); ok {
			if s2, ok := b.AsString(); ok {
				lowerCmp := strings.Compare(strings.ToLower(s1), strings.ToLower(s2))
				if lowerCmp != 0 {
					return lowerCmp
				}
				if s1 != s2 {
					rank1 := caseRank(s1)
					rank2 := caseRank(s2)
					if rank1 != rank2 {
						if rank1 < rank2 {
							return -1
						}
						return 1
					}
					return strings.Compare(s1, s2)
				}
				return 0
			}
		}
	}
	if cmp, ok := a.Compare(b); ok {
		return cmp
	}
	return strings.Compare(a.Repr(), b.Repr())
}

func groupByEqual(a, b value.Value, caseSensitive bool) bool {
	if !caseSensitive {
		if s1, ok := a.AsString(); ok {
			if s2, ok := b.AsString(); ok {
				return strings.EqualFold(s1, s2)
			}
		}
	}
	if cmp, ok := a.Compare(b); ok {
		return cmp == 0
	}
	return a.Repr() == b.Repr()
}

func caseRank(s string) int {
	if s == strings.ToLower(s) {
		return 1
	}
	return 0
}

// groupObject represents a group in groupby filter
type groupObject struct {
	grouper value.Value
	list    []value.Value
}

func (g *groupObject) GetAttr(name string) value.Value {
	switch name {
	case "grouper":
		return g.grouper
	case "list":
		return value.FromSlice(g.list)
	}
	return value.Undefined()
}

func (g *groupObject) Iter() []value.Value {
	return []value.Value{g.grouper, value.FromSlice(g.list)}
}

func (g *groupObject) Len() (int, bool) {
	return 2, true
}

func (g *groupObject) GetItem(key value.Value) value.Value {
	if idx, ok := key.AsInt(); ok {
		switch idx {
		case 0:
			return g.grouper
		case 1:
			return value.FromSlice(g.list)
		}
	}
	return value.Undefined()
}

func (g *groupObject) String() string {
	listRepr := value.FromSlice(g.list).Repr()
	return fmt.Sprintf("[%s, %s]", g.grouper.Repr(), listRepr)
}

func filterChain(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	allValues := append([]value.Value{val}, args...)

	allMaps := true
	allSeq := true
	for _, v := range allValues {
		if _, ok := v.AsMap(); !ok {
			allMaps = false
		}
		if _, ok := v.AsSlice(); !ok {
			allSeq = false
		}
	}

	if allMaps {
		merged := make(map[string]value.Value)
		for _, v := range allValues {
			m, _ := v.AsMap()
			for k, val := range m {
				merged[k] = val
			}
		}
		return value.FromMap(merged), nil
	}

	// Get items from first value
	items := val.Iter()
	if items == nil {
		items = []value.Value{}
	}

	// Chain all arguments
	for _, arg := range args {
		argItems := arg.Iter()
		if argItems != nil {
			items = append(items, argItems...)
		}
	}

	if allSeq {
		return value.FromSlice(items), nil
	}

	// Return as iterable to support length, indexing, etc.
	return value.FromObject(&chainObject{items: items}), nil
}

// chainObject allows chained iterables to support length and indexing
type chainObject struct {
	items []value.Value
}

func (c *chainObject) GetAttr(name string) value.Value {
	return value.Undefined()
}

func (c *chainObject) Iter() []value.Value {
	return c.items
}

func (c *chainObject) Len() (int, bool) {
	return len(c.items), true
}

func (c *chainObject) GetItem(key value.Value) value.Value {
	if idx, ok := key.AsInt(); ok {
		if idx < 0 {
			idx = int64(len(c.items)) + idx
		}
		if idx >= 0 && idx < int64(len(c.items)) {
			return c.items[idx]
		}
	}
	return value.Undefined()
}

func filterZip(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	// Collect all sequences
	seqs := [][]value.Value{val.Iter()}
	for _, arg := range args {
		seqs = append(seqs, arg.Iter())
	}

	// Find minimum length
	minLen := math.MaxInt
	for _, seq := range seqs {
		if seq == nil {
			minLen = 0
			break
		}
		if len(seq) < minLen {
			minLen = len(seq)
		}
	}

	if minLen == 0 || minLen == math.MaxInt {
		return value.FromSlice(nil), nil
	}

	// Zip
	result := make([]value.Value, minLen)
	for i := 0; i < minLen; i++ {
		tuple := make([]value.Value, len(seqs))
		for j, seq := range seqs {
			tuple[j] = seq[i]
		}
		result[i] = value.FromSlice(tuple)
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
	if b, ok := val.AsBool(); ok {
		if b {
			return value.FromInt(1), nil
		}
		return value.FromInt(0), nil
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
	if b, ok := val.AsBool(); ok {
		if b {
			return value.FromFloat(1.0), nil
		}
		return value.FromFloat(0.0), nil
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

	multiplier := math.Pow(10, float64(precision))

	switch method {
	case "floor":
		f = math.Floor(f*multiplier) / multiplier
	case "ceil":
		f = math.Ceil(f*multiplier) / multiplier
	default: // common
		f = math.Round(f*multiplier) / multiplier
	}

	if precision == 0 {
		if val.IsActualFloat() {
			return value.FromFloat(f), nil
		}
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

		caseSensitive := false
		if cs, ok := kwargs["case_sensitive"]; ok {
			if b, ok := cs.AsBool(); ok {
				caseSensitive = b
			}
		}

		cmpValues := func(a, b value.Value) int {
			if !caseSensitive {
				if s1, ok := a.AsString(); ok {
					if s2, ok := b.AsString(); ok {
						lowerCmp := strings.Compare(strings.ToLower(s1), strings.ToLower(s2))
						if lowerCmp != 0 {
							return lowerCmp
						}
						return strings.Compare(s1, s2)
					}
				}
			}
			if cmp, ok := a.Compare(b); ok {
				return cmp
			}
			return strings.Compare(a.Repr(), b.Repr())
		}

		if byValue {
			sort.Slice(keys, func(i, j int) bool {
				cmp := cmpValues(m[keys[i]], m[keys[j]])
				if reverse {
					return cmp > 0
				}
				return cmp < 0
			})
		} else {
			sort.Slice(keys, func(i, j int) bool {
				cmp := cmpValues(value.FromString(keys[i]), value.FromString(keys[j]))
				if reverse {
					return cmp > 0
				}
				return cmp < 0
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
	return value.FromString(pprintValue(val, 0)), nil
}

func pprintValue(val value.Value, indent int) string {
	pad := strings.Repeat(" ", indent)
	switch val.Kind() {
	case value.KindSeq:
		items, _ := val.AsSlice()
		if len(items) == 0 {
			return "[]"
		}
		var sb strings.Builder
		sb.WriteString("[\n")
		for _, item := range items {
			sb.WriteString(strings.Repeat(" ", indent+4))
			sb.WriteString(pprintValue(item, indent+4))
			sb.WriteString(",\n")
		}
		sb.WriteString(pad)
		sb.WriteString("]")
		return sb.String()
	case value.KindMap:
		m, _ := val.AsMap()
		if len(m) == 0 {
			return "{}"
		}
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		var sb strings.Builder
		sb.WriteString("{\n")
		for _, k := range keys {
			sb.WriteString(strings.Repeat(" ", indent+4))
			sb.WriteString(fmt.Sprintf("%q: %s,", k, pprintValue(m[k], indent+4)))
			sb.WriteString("\n")
		}
		sb.WriteString(pad)
		sb.WriteString("}")
		return sb.String()
	default:
		return val.Repr()
	}
}

func filterTojson(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	// Convert value to Go native type for JSON serialization
	native := valueToNative(val)

	// Check for indent option
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
	jsonStr := string(data)
	jsonStr = strings.ReplaceAll(jsonStr, "'", "\\u0027")
	return value.FromSafeString(jsonStr), nil
}

func valueToNative(v value.Value) interface{} {
	switch v.Kind() {
	case value.KindUndefined, value.KindNone:
		return nil
	case value.KindBool:
		b, _ := v.AsBool()
		return b
	case value.KindNumber:
		if i, ok := v.AsInt(); ok && v.IsActualInt() {
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
		if m, ok := v.AsMap(); ok {
			result := make(map[string]interface{})
			for k, val := range m {
				result[k] = valueToNative(val)
			}
			return result
		}
		return v.String()
	}
}

func urlencodeString(input string) string {
	escaped := url.QueryEscape(input)
	escaped = strings.ReplaceAll(escaped, "+", "%20")
	escaped = strings.ReplaceAll(escaped, "%2F", "/")
	return escaped
}

func filterUrlencode(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	// Check if it's a map (dict) - encode as query string
	if m, ok := val.AsMap(); ok {
		var parts []string
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			v := m[k]
			if v.IsNone() {
				continue
			}
			parts = append(parts, urlencodeString(k)+"="+urlencodeString(v.String()))
		}
		return value.FromString(strings.Join(parts, "&")), nil
	}

	s, ok := val.AsString()
	if !ok {
		s = val.String()
	}
	return value.FromString(urlencodeString(s)), nil
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
