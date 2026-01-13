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

// FilterUpper converts a value to uppercase.
//
// This filter converts the entire string to uppercase characters.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("upper", FilterUpper)
//
// Template usage:
//
//	<h1>{{ chapter.title|upper }}</h1>
//
// Note: This filter only works on string values. Non-string values are returned
// unchanged.
func FilterUpper(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		return value.FromString(strings.ToUpper(s)), nil
	}
	return val, nil
}

// FilterLower converts a value to lowercase.
//
// This filter converts the entire string to lowercase characters.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("lower", FilterLower)
//
// Template usage:
//
//	<h1>{{ chapter.title|lower }}</h1>
func FilterLower(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		return value.FromString(strings.ToLower(s)), nil
	}
	return val, nil
}

// FilterCapitalize converts the first character to uppercase and the rest to lowercase.
//
// This filter converts a string by uppercasing only the first character and
// lowercasing all remaining characters. This is different from FilterTitle which
// capitalizes the first letter of each word.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("capitalize", FilterCapitalize)
//
// Template usage:
//
//	{{ "hello WORLD"|capitalize }}
//	  -> "Hello world"
func FilterCapitalize(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterTitle converts a value to title case.
//
// This filter converts a string to title case by capitalizing the first letter
// of each word and lowercasing all other letters. Words are defined as sequences
// of characters separated by whitespace or common punctuation.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("title", FilterTitle)
//
// Template usage:
//
//	<h1>{{ chapter.title|title }}</h1>
//	{{ "hello world"|title }}
//	  -> "Hello World"
func FilterTitle(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
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

// FilterTrim strips leading and trailing characters from a string.
//
// By default, it strips whitespace characters. You can optionally provide
// a string of characters to trim as the first argument.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("trim", FilterTrim)
//
// Template usage:
//
//	{{ "  hello  "|trim }}
//	  -> "hello"
//	{{ "xxxhelloxxx"|trim("x") }}
//	  -> "hello"
func FilterTrim(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterReplace replaces occurrences of a substring with another string.
//
// This filter replaces all occurrences of the first parameter with the second.
// Optionally, you can provide a third parameter to limit the number of replacements.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("replace", FilterReplace)
//
// Template usage:
//
//	{{ "Hello World"|replace("Hello", "Goodbye") }}
//	  -> "Goodbye World"
//	{{ "aaa"|replace("a", "b", 2) }}
//	  -> "bba"
func FilterReplace(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterDefault provides a default value if the input is undefined or none.
//
// If the value is undefined or none, it returns the provided default value.
// Setting the optional second parameter to true will also treat empty/falsy
// values as undefined.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("default", FilterDefault)
//
// Template usage:
//
//	{{ my_variable|default("default value") }}
//	{{ ""|default("empty", true) }}
//	  -> "empty"
//
// Keyword arguments:
//   - default: The default value to use
//   - boolean: If true, treat falsy values as undefined
func FilterDefault(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterSafe marks a value as safe for auto-escaping.
//
// When a value is marked as safe, it will not be automatically escaped
// when rendered in templates with auto-escaping enabled.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("safe", FilterSafe)
//
// Template usage:
//
//	{{ html_content|safe }}
//
// Warning: Only use this filter on values you trust to contain safe HTML.
// Using it on untrusted content can lead to security vulnerabilities.
func FilterSafe(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if s, ok := val.AsString(); ok {
		return value.FromSafeString(s), nil
	}
	return value.FromSafeString(val.String()), nil
}

// FilterEscape escapes a string for safe HTML output.
//
// By default, this filter is also registered under the alias "e". If the value
// is already marked as safe, it is returned unchanged. Otherwise, it escapes
// HTML special characters.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("escape", FilterEscape)
//
// Template usage:
//
//	{{ user_input|escape }}
//	{{ "<script>alert('xss')</script>"|e }}
//	  -> "&lt;script&gt;alert('xss')&lt;/script&gt;"
func FilterEscape(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsSafe() {
		return val, nil
	}
	if s, ok := val.AsString(); ok {
		return value.FromSafeString(EscapeHTML(s)), nil
	}
	return value.FromSafeString(EscapeHTML(val.String())), nil
}

// FilterString converts a value into a string if it's not one already.
//
// If the value is already a string (and marked as safe if applicable),
// that value is preserved. Otherwise, the value is converted to its
// string representation.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("string", FilterString)
//
// Template usage:
//
//	{{ 42|string }}
//	  -> "42"
func FilterString(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.Kind() == value.KindString {
		return val, nil
	}
	return value.FromString(val.String()), nil
}

// FilterBool converts a value into a boolean.
//
// This filter evaluates the truthiness of a value according to MiniJinja's
// rules: non-zero numbers, non-empty strings, and non-empty collections
// are true.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("bool", FilterBool)
//
// Template usage:
//
//	{{ 42|bool }}
//	  -> true
//	{{ ""|bool }}
//	  -> false
func FilterBool(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	return value.FromBool(val.IsTrue()), nil
}

// FilterSplit splits a string into a list of substrings.
//
// If no split pattern is provided or it's none, the string is split on
// whitespace with multiple spaces removed. Otherwise, the string is split
// using the provided separator.
//
// The optional second parameter defines the maximum number of splits
// (following Python conventions where 1 means one split and two resulting items).
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("split", FilterSplit)
//
// Template usage:
//
//	{{ "hello world"|split }}
//	  -> ["hello", "world"]
//	{{ "a,b,c"|split(",") }}
//	  -> ["a", "b", "c"]
//	{{ "a,b,c,d"|split(",", 2) }}
//	  -> ["a", "b", "c,d"]
func FilterSplit(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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
			maxSplits = int(m) + 1
		}
	}

	var parts []string
	if splitOn == nil {
		// Split on whitespace
		if maxSplits <= 0 {
			parts = strings.Fields(s)
		} else {
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

// FilterLines splits a string into lines.
//
// The newline character is removed in the process. This function supports
// both Windows (CRLF) and UNIX (LF) style newlines.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("lines", FilterLines)
//
// Template usage:
//
//	{{ "foo\nbar\nbaz"|lines }}
//	  -> ["foo", "bar", "baz"]
func FilterLines(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterLength returns the number of items in a collection or string.
//
// This filter works on sequences, maps, and strings. For strings, it returns
// the number of characters. This filter is also commonly available under the
// alias "count".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("length", FilterLength)
//
// Template usage:
//
//	<p>{{ users|length }} users found</p>
//	{{ "hello"|length }}
//	  -> 5
func FilterLength(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if l, ok := val.Len(); ok {
		return value.FromInt(int64(l)), nil
	}
	return value.FromInt(0), nil
}

// FilterFirst returns the first item from an iterable.
//
// If the iterable is empty, undefined is returned.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("first", FilterFirst)
//
// Template usage:
//
//	<dl>
//	  <dt>primary email
//	  <dd>{{ user.email_addresses|first|default('no email') }}
//	</dl>
func FilterFirst(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if len(items) > 0 {
		return items[0], nil
	}
	return value.Undefined(), nil
}

// FilterLast returns the last item from an iterable.
//
// If the iterable is empty, undefined is returned.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("last", FilterLast)
//
// Template usage:
//
//	<h2>Most Recent Update</h2>
//	{% with update = updates|last %}
//	  <dl>
//	    <dt>Location
//	    <dd>{{ update.location }}
//	    <dt>Status
//	    <dd>{{ update.status }}
//	  </dl>
//	{% endwith %}
func FilterLast(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if len(items) > 0 {
		return items[len(items)-1], nil
	}
	return value.Undefined(), nil
}

// FilterReverse reverses an iterable or string.
//
// For strings, this reverses the characters. For iterables, it reverses
// the order of items.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("reverse", FilterReverse)
//
// Template usage:
//
//	{% for user in users|reverse %}
//	  <li>{{ user.name }}
//	{% endfor %}
//	{{ "hello"|reverse }}
//	  -> "olleh"
func FilterReverse(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterSort sorts an iterable.
//
// The filter accepts several keyword arguments to control sorting behavior:
//
//   - reverse: set to true to sort in descending order
//   - case_sensitive: set to true for case-sensitive string sorting (default: false)
//   - attribute: can be set to an attribute name or dotted path to sort by that attribute
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("sort", FilterSort)
//
// Template usage:
//
//	{{ [3, 1, 2]|sort }}
//	  -> [1, 2, 3]
//	{{ users|sort(attribute="age") }}
//	{{ users|sort(attribute="age", reverse=true) }}
//	{{ cities|sort(attribute="name, state") }}
func FilterSort(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterJoin concatenates items from an iterable into a string.
//
// The optional first parameter is the separator string to use between items.
// If not provided, items are concatenated directly without a separator.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("join", FilterJoin)
//
// Template usage:
//
//	{{ ["a", "b", "c"]|join(", ") }}
//	  -> "a, b, c"
//	{{ items|join }}
func FilterJoin(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterList converts a value into a list.
//
// If the value is already a list, it's returned unchanged. For maps, this
// returns a list of keys. For strings, this returns the characters. If the
// value is undefined, an empty list is returned.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("list", FilterList)
//
// Template usage:
//
//	{{ "abc"|list }}
//	  -> ["a", "b", "c"]
//	{{ range(5)|list }}
//	  -> [0, 1, 2, 3, 4]
func FilterList(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	items := val.Iter()
	if items != nil {
		return value.FromSlice(items), nil
	}
	return value.FromSlice(nil), nil
}

// FilterUnique returns unique items from an iterable.
//
// The unique items are yielded in the same order as their first occurrence.
// The filter will not detect duplicate objects or arrays, only primitives.
//
// Keyword arguments:
//   - case_sensitive: set to true for case-sensitive comparison (default: false)
//   - attribute: operate on an attribute instead of the value itself
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("unique", FilterUnique)
//
// Template usage:
//
//	{{ ["a", "b", "a", "c"]|unique }}
//	  -> ["a", "b", "c"]
//	{{ users|unique(attribute="city") }}
func FilterUnique(_ *State, val value.Value, _ []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterMin returns the smallest item from an iterable.
//
// If the iterable is empty, undefined is returned.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("min", FilterMin)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|min }}
//	  -> 1
func FilterMin(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterMax returns the largest item from an iterable.
//
// If the iterable is empty, undefined is returned.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("max", FilterMax)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|max }}
//	  -> 4
func FilterMax(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterSum sums up all numeric values in an iterable.
//
// The optional first parameter provides a start value (default is 0).
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("sum", FilterSum)
//
// Template usage:
//
//	{{ [1, 2, 3]|sum }}
//	  -> 6
//	{{ values|sum(100) }}
//	  -> sum of values + 100
func FilterSum(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterBatch batches items into groups of a given size.
//
// This filter works like FilterSlice but in the other direction. It returns
// a list of lists with the given number of items. If you provide a second
// parameter, it's used to fill up missing items in the last batch.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("batch", FilterBatch)
//
// Template usage:
//
//	<table>
//	{% for row in items|batch(3, " ") %}
//	  <tr>
//	  {% for column in row %}
//	    <td>{{ column }}</td>
//	  {% endfor %}
//	  </tr>
//	{% endfor %}
//	</table>
//
// Keyword arguments:
//   - fill_with: value to use for filling incomplete batches
func FilterBatch(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterSlice slices an iterable into a given number of columns.
//
// This filter works like FilterBatch but slices into columns instead of rows.
// If you pass a second argument, it's used to fill missing values on the
// last iteration.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("slice", FilterSlice)
//
// Template usage:
//
//	<div class="columnwrapper">
//	{% for column in items|slice(3) %}
//	  <ul class="column-{{ loop.index }}">
//	  {% for item in column %}
//	    <li>{{ item }}</li>
//	  {% endfor %}
//	  </ul>
//	{% endfor %}
//	</div>
func FilterSlice(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterMap applies a filter to a sequence or looks up an attribute.
//
// This is useful when dealing with lists of objects where you're only
// interested in a specific value. You can either map an attribute or apply
// a filter to each item.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("map", FilterMap)
//
// Template usage (attribute mapping):
//
//	{{ users|map(attribute="username")|join(", ") }}
//	{{ users|map(attribute="address.city", default="Unknown")|join }}
//
// Template usage (filter mapping):
//
//	{{ titles|map("lower")|join(", ") }}
//
// Keyword arguments:
//   - attribute: name or dotted path of attribute to extract
//   - default: value to use when attribute is missing
func FilterMap(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterSelect filters a sequence by applying a test.
//
// This creates a new sequence containing only values that pass the test.
// If no test is specified, items are evaluated for truthiness.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("select", FilterSelect)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|select("odd") }}
//	  -> [1, 3]
//	{{ [false, null, 42]|select }}
//	  -> [42]
func FilterSelect(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterReject filters a sequence by rejecting values that pass a test.
//
// This is the inverse of FilterSelect - it creates a new sequence containing
// only values that fail the test.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("reject", FilterReject)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|reject("odd") }}
//	  -> [2, 4]
func FilterReject(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterSelectAttr filters a sequence by testing an attribute.
//
// This is like FilterSelect but tests an attribute of each object instead
// of the object itself.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("selectattr", FilterSelectAttr)
//
// Template usage:
//
//	{{ users|selectattr("is_active") }}
//	  -> all users where x.is_active is true
//	{{ users|selectattr("id", "even") }}
//	  -> users with even IDs
func FilterSelectAttr(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterRejectAttr filters a sequence by rejecting items where an attribute passes a test.
//
// This is like FilterReject but tests an attribute of each object instead
// of the object itself.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("rejectattr", FilterRejectAttr)
//
// Template usage:
//
//	{{ users|rejectattr("is_active") }}
//	  -> all users where x.is_active is false
//	{{ users|rejectattr("id", "even") }}
//	  -> users with odd IDs
func FilterRejectAttr(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterGroupBy groups a sequence of objects by a common attribute.
//
// The attribute can use dot notation for nested access. Items are automatically
// sorted first. Each group is returned as a tuple/object with "grouper" and "list"
// attributes.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("groupby", FilterGroupBy)
//
// Template usage:
//
//	<ul>{% for city, items in users|groupby("city") %}
//	  <li>{{ city }}
//	    <ul>{% for user in items %}
//	      <li>{{ user.name }}
//	    {% endfor %}</ul>
//	  </li>
//	{% endfor %}</ul>
//
// Keyword arguments:
//   - attribute: name or dotted path of attribute to group by
//   - default: value to use when attribute is missing
//   - case_sensitive: if true, sort in a case-sensitive manner (default: false)
func FilterGroupBy(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterChain chains multiple iterables into a single iterable.
//
// If all objects are maps, the result acts like a merged map (with later
// values overriding earlier ones for duplicate keys). If all objects are
// sequences, the result acts like an appended list. Otherwise, it creates
// a chained iterator.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("chain", FilterChain)
//
// Template usage:
//
//	{{ list1|chain(list2, list3)|length }}
//	{% for user in shard0|chain(shard1, shard2) %}
//	  {{ user.name }}
//	{% endfor %}
func FilterChain(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterZip zips multiple iterables into tuples.
//
// This works like Python's zip function. It takes one or more iterables and
// returns an iterable of tuples where each tuple contains one element from
// each input. Iteration stops when the shortest iterable is exhausted.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("zip", FilterZip)
//
// Template usage:
//
//	{{ [1, 2, 3]|zip(["a", "b", "c"]) }}
//	  -> [(1, "a"), (2, "b"), (3, "c")]
//	{{ [1, 2]|zip(["a", "b", "c"], ["x", "y", "z"]) }}
//	  -> [(1, "a", "x"), (2, "b", "y")]
func FilterZip(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterAbs returns the absolute value of a number.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("abs", FilterAbs)
//
// Template usage:
//
//	{{ -42|abs }}
//	  -> 42
//	{{ 3.14|abs }}
//	  -> 3.14
func FilterAbs(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterInt converts a value to an integer.
//
// String values are parsed as integers. Float values are truncated.
// Boolean true becomes 1, false becomes 0. If conversion fails, the
// optional default value is returned (default: 0).
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("int", FilterInt)
//
// Template usage:
//
//	{{ "42"|int }}
//	  -> 42
//	{{ "invalid"|int(default=0) }}
//	  -> 0
//
// Keyword arguments:
//   - default: value to return if conversion fails
func FilterInt(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterFloat converts a value to a float.
//
// String values are parsed as floats. Integer values are converted to floats.
// Boolean true becomes 1.0, false becomes 0.0. If conversion fails, the
// optional default value is returned (default: 0.0).
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("float", FilterFloat)
//
// Template usage:
//
//	{{ "42.5"|float }}
//	  -> 42.5
//	{{ "invalid"|float(default=0.0) }}
//	  -> 0.0
//
// Keyword arguments:
//   - default: value to return if conversion fails
func FilterFloat(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterRound rounds a number to a given precision.
//
// The first parameter specifies the precision (default is 0). The second
// optional parameter specifies the rounding method: "common" (default),
// "floor", or "ceil".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("round", FilterRound)
//
// Template usage:
//
//	{{ 42.55|round }}
//	  -> 43
//	{{ 42.55|round(1) }}
//	  -> 42.6
//	{{ 42.55|round(method="floor") }}
//	  -> 42
//
// Keyword arguments:
//   - precision: number of decimal places (default: 0)
//   - method: rounding method - "common", "floor", or "ceil" (default: "common")
func FilterRound(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterItems returns an iterable of key-value pairs from a map.
//
// This converts a map into a list of [key, value] tuples. The keys are
// sorted alphabetically.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("items", FilterItems)
//
// Template usage:
//
//	<dl>
//	{% for key, value in my_dict|items %}
//	  <dt>{{ key }}
//	  <dd>{{ value }}
//	{% endfor %}
//	</dl>
func FilterItems(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterKeys returns a list of keys from a map.
//
// The keys are sorted alphabetically.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("keys", FilterKeys)
//
// Template usage:
//
//	{{ my_dict|keys }}
func FilterKeys(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterValues returns a list of values from a map.
//
// The values are returned in the same order as the sorted keys.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("values", FilterValues)
//
// Template usage:
//
//	{{ my_dict|values }}
func FilterValues(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterDictSort sorts a map by keys or values.
//
// Returns a list of [key, value] pairs sorted by key (default) or by value.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("dictsort", FilterDictSort)
//
// Template usage:
//
//	{% for key, value in my_dict|dictsort %}
//	  {{ key }}: {{ value }}
//	{% endfor %}
//
// Keyword arguments:
//   - by: set to "value" to sort by value instead of key (default: "key")
//   - reverse: set to true to sort in descending order
//   - case_sensitive: set to true for case-sensitive sorting (default: false)
func FilterDictSort(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterAttr looks up an attribute by name.
//
// This is equivalent to using the [] operator in MiniJinja. It's provided
// for compatibility with Jinja2.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("attr", FilterAttr)
//
// Template usage:
//
//	{{ value|attr("key") }}
//	  -> same as value["key"] or value.key
func FilterAttr(_ *State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	if len(args) < 1 {
		return value.Undefined(), fmt.Errorf("attr filter requires attribute name")
	}
	name, _ := args[0].AsString()
	return val.GetAttr(name), nil
}

// FilterIndent indents each line of a string with spaces.
//
// The first parameter sets the indentation width (default: 4). The second
// optional parameter determines whether to indent the first line (default: false).
// The third optional parameter determines whether to indent blank lines (default: false).
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("indent", FilterIndent)
//
// Template usage:
//
//	config:
//	{{ yaml_content|indent(2) }}
//	{{ yaml_content|indent(2, true) }}
//	{{ yaml_content|indent(2, true, true) }}
//
// Keyword arguments:
//   - width: number of spaces to indent (default: 4)
//   - first: whether to indent the first line (default: false)
//   - blank: whether to indent blank lines (default: false)
func FilterIndent(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterPprint pretty-prints a value for debugging.
//
// This is useful for debugging templates as it formats values in a more
// readable way than the default string representation.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("pprint", FilterPprint)
//
// Template usage:
//
//	<pre>{{ complex_object|pprint }}</pre>
func FilterPprint(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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

// FilterTojson serializes a value to JSON.
//
// The resulting value is safe to use in HTML as special characters are escaped.
// The optional parameter controls indentation: true for 2 spaces, or an integer
// for custom indentation.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("tojson", FilterTojson)
//
// Template usage:
//
//	<script>
//	  const CONFIG = {{ config|tojson }};
//	</script>
//	<a href="#" data-info='{{ info|tojson }}'>...</a>
//	{{ data|tojson(indent=2) }}
//
// Keyword arguments:
//   - indent: true for 2-space indent, or integer for custom indent
func FilterTojson(_ *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

// FilterUrlencode URL-encodes a value.
//
// If given a map, it encodes the parameters into a query string. Otherwise,
// it encodes the stringified value. None and undefined values in maps are
// skipped.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("urlencode", FilterUrlencode)
//
// Template usage:
//
//	<a href="/search?{{ {"q": "my search", "lang": "en"}|urlencode }}">
//	{{ "hello world"|urlencode }}
//	  -> "hello%20world"
func FilterUrlencode(_ *State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
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
