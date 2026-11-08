// Package filters provides MiniJinja's built-in filters.
package filters

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"math/big"
	"net/url"
	"slices"
	"sort"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"

	mjerrors "github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/errors"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/fuel"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/jsonvalue"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// State provides access to the runtime context for filters and tests.
//
// It is implemented by *minijinja.State and exposes helper methods for
// looking up filters and tests by name.
type State interface {
	Context() context.Context
	Lookup(name string) value.Value
	Name() string
	GetFilter(name string) (FilterFunc, bool)
	GetTest(name string) (TestFunc, bool)
}

// FilterFunc is the signature for filter functions.
type FilterFunc func(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error)

// TestFunc is the signature for test functions.
type TestFunc func(state State, val value.Value, args []value.Value) (bool, error)

type undefinedBehaviorProvider interface {
	UndefinedBehavior() value.UndefinedBehavior
}

func undefinedBehavior(state State) value.UndefinedBehavior {
	if state == nil {
		return value.UndefinedLenient
	}
	if provider, ok := state.(undefinedBehaviorProvider); ok {
		return provider.UndefinedBehavior()
	}
	return value.UndefinedLenient
}

type autoEscapeProvider interface {
	AutoEscapeEnabled() bool
}

type valueFormatter interface {
	Format(value.Value) (string, error)
}

type escapedValueFormatter interface {
	FormatEscaped(value.Value) (string, error)
}

func autoEscapeEnabled(state State) bool {
	provider, ok := state.(autoEscapeProvider)
	return ok && provider.AutoEscapeEnabled()
}

func formatForSafeOutput(state State, val value.Value, forceEscape bool) (string, error) {
	if val.IsSafe() {
		return val.String(), nil
	}
	if forceEscape {
		if formatter, ok := state.(escapedValueFormatter); ok {
			return formatter.FormatEscaped(val)
		}
		if !autoEscapeEnabled(state) {
			return EscapeHTML(val.String()), nil
		}
	}
	if formatter, ok := state.(valueFormatter); ok {
		return formatter.Format(val)
	}
	return EscapeHTML(val.String()), nil
}

// stringInput returns the string a string filter works on. Like StringInput in
// the Rust implementation, a value that is not a string is converted to its
// string representation (`true` becomes "True", `none` becomes "None") and
// undefined values fail with strict and semi-strict undefined behavior.
func stringInput(state State, val value.Value) (string, error) {
	if err := assertNotUndefined(state, val); err != nil {
		return "", err
	}
	if s, ok := val.AsString(); ok {
		return s, nil
	}
	return val.String(), nil
}

// stringOrBytesInput returns the string for string and bytes values. Like
// TryFrom<Value> for Arc<str> in Rust, other values (including none and
// undefined) are rejected.
func stringOrBytesInput(val value.Value) (string, error) {
	if s, ok := val.AsString(); ok {
		return s, nil
	}
	if bytes, isBytes := val.Raw().([]byte); isBytes {
		return strings.ToValidUTF8(string(bytes), "\uFFFD"), nil
	}
	return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, "value is not a string")
}

// mapPairs returns the key-value pairs of a map value. Maps created by
// templates are returned in insertion order, plain Go maps with sorted keys
// and map objects in their iteration order. The second return value is false
// if the value is not a map.
func mapPairs(state State, val value.Value) ([][2]value.Value, bool, error) {
	if m, ok := val.Raw().(*value.Map); ok {
		if err := chargeItems(state, m.Len()); err != nil {
			return nil, true, err
		}
		pairs := make([][2]value.Value, 0, m.Len())
		for k, v := range m.All() {
			pairs = append(pairs, [2]value.Value{k, v})
		}
		return pairs, true, nil
	}
	if m, ok := val.AsMap(); ok {
		if err := chargeItems(state, len(m)); err != nil {
			return nil, true, err
		}
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		pairs := make([][2]value.Value, len(keys))
		for i, k := range keys {
			pairs[i] = [2]value.Value{value.FromString(k), m[k]}
		}
		return pairs, true, nil
	}
	if val.Kind() != value.KindMap {
		return nil, false, nil
	}
	keys, err := fuelOf(state).Items(val)
	if err != nil {
		return nil, true, err
	}
	pairs := make([][2]value.Value, len(keys))
	for i, k := range keys {
		pairs[i] = [2]value.Value{k, val.GetItem(k)}
	}
	return pairs, true, nil
}

var minI128 = new(big.Int).Neg(new(big.Int).Lsh(big.NewInt(1), 127))
var maxI128 = new(big.Int).Sub(new(big.Int).Lsh(big.NewInt(1), 127), big.NewInt(1))

func fitsI128(n *big.Int) bool {
	return n.Cmp(minI128) >= 0 && n.Cmp(maxI128) <= 0
}

func bigIntValue(n *big.Int) value.Value {
	if n.IsInt64() {
		return value.FromInt(n.Int64())
	}
	return value.FromBigInt(n)
}

// asBigInt returns a number stored as a big integer (an integer outside the
// int64 range) as a big.Int.
func asBigInt(val value.Value) (*big.Int, bool) {
	if val.Kind() != value.KindNumber || val.IsActualInt() || val.IsActualFloat() {
		return nil, false
	}
	return new(big.Int).SetString(val.String(), 10)
}

// parseFloat parses a float like Rust's `str::parse::<f64>`, which gives an
// infinity for numbers too large for a float instead of failing.  Unlike
// strconv.ParseFloat it does not accept underscores or hexadecimal floats but
// accepts a sign in front of NaN.
func parseFloat(s string) (float64, error) {
	unsigned := s
	if strings.HasPrefix(s, "+") || strings.HasPrefix(s, "-") {
		unsigned = s[1:]
	}
	if strings.EqualFold(unsigned, "nan") {
		return math.NaN(), nil
	}
	if strings.Contains(s, "_") || (len(unsigned) >= 2 && unsigned[0] == '0' && (unsigned[1] == 'x' || unsigned[1] == 'X')) {
		return 0, &strconv.NumError{Func: "ParseFloat", Num: s, Err: strconv.ErrSyntax}
	}
	f, err := strconv.ParseFloat(s, 64)
	if numErr, ok := err.(*strconv.NumError); ok && numErr.Err == strconv.ErrRange {
		return f, nil
	}
	return f, err
}

// floatToInt converts a float to an integer like `f as i128` does in Rust:
// it truncates toward zero, saturates at the bounds of an i128 and turns NaN
// into 0.
func floatToInt(f float64) value.Value {
	switch {
	case math.IsNaN(f):
		return value.FromInt(0)
	case f >= -0x1p63 && f < 0x1p63:
		return value.FromInt(int64(f))
	case f >= 0x1p127:
		return value.FromBigInt(new(big.Int).Set(maxI128))
	case f <= -0x1p127:
		return value.FromBigInt(new(big.Int).Set(minI128))
	}
	n, _ := big.NewFloat(f).Int(nil)
	return value.FromBigInt(n)
}

func preserveSafety(input value.Value, output string) value.Value {
	if input.IsSafe() {
		return value.FromSafeString(output)
	}
	return value.FromString(output)
}

// FilterUpper converts a value to uppercase.
//
// This filter converts the entire string to uppercase characters.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("upper", FilterUpper)
//
// Template usage:
//
//	<h1>{{ chapter.title|upper }}</h1>
//
// Values that are not strings are converted to strings first.
func FilterUpper(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	return preserveSafety(val, toUpper(s)), nil
}

// FilterLower converts a value to lowercase.
//
// This filter converts the entire string to lowercase characters.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("lower", FilterLower)
//
// Template usage:
//
//	<h1>{{ chapter.title|lower }}</h1>
func FilterLower(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	return preserveSafety(val, toLower(s)), nil
}

// FilterCapitalize converts the first character to uppercase and the rest to lowercase.
//
// This filter converts a string by uppercasing only the first character and
// lowercasing all remaining characters. This is different from FilterTitle which
// capitalizes the first letter of each word.  Like in Python, the first
// character is converted to its titlecase form, so "ǆ" becomes "ǅ" and "ß"
// becomes "Ss".
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("capitalize", FilterCapitalize)
//
// Template usage:
//
//	{{ "hello WORLD"|capitalize }}
//	  -> "Hello world"
func FilterCapitalize(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	var output strings.Builder
	if first, size := utf8.DecodeRuneInString(s); size > 0 {
		output.Grow(len(s))
		writeTitle(&output, first)
		writeLowerTail(&output, s, size)
	}
	return preserveSafety(val, output.String()), nil
}

// FilterTitle converts a value to title case.
//
// This filter converts a string to title case by capitalizing the first letter
// of each word and lowercasing all other letters. Words are defined as sequences
// of characters separated by whitespace or common punctuation.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("title", FilterTitle)
//
// Template usage:
//
//	<h1>{{ chapter.title|title }}</h1>
//	{{ "hello world"|title }}
//	  -> "Hello World"
func FilterTitle(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	var result strings.Builder
	result.Grow(len(s))
	capitalizeNext := true
	skipUntil := 0
	for i, r := range s {
		if i < skipUntil {
			continue
		}
		if isTitleWordSeparator(r) {
			capitalizeNext = true
			result.WriteRune(r)
		} else if capitalizeNext {
			writeUpper(&result, r)
			capitalizeNext = false
		} else if r < utf8.RuneSelf {
			if 'A' <= r && r <= 'Z' {
				r += 'a' - 'A'
			}
			result.WriteByte(byte(r))
		} else if r == 'Σ' {
			// Whether a capital sigma lowercases to a final sigma depends
			// on the rest of the word, so lowercase the rest of the word
			// at once.
			wordStart := strings.LastIndexFunc(s[:i], isTitleWordSeparator)
			if wordStart >= 0 {
				_, size := utf8.DecodeRuneInString(s[wordStart:])
				wordStart += size
			} else {
				wordStart = 0
			}
			wordEnd := strings.IndexFunc(s[i:], isTitleWordSeparator)
			if wordEnd < 0 {
				wordEnd = len(s)
			} else {
				wordEnd += i
			}
			writeLowerTail(&result, s[wordStart:wordEnd], i-wordStart)
			skipUntil = wordEnd
		} else {
			writeLowerRune(&result, r)
		}
	}
	return value.FromString(result.String()), nil
}

// isTitleWordSeparator reports whether r starts a new word for FilterTitle.
func isTitleWordSeparator(r rune) bool {
	return unicode.IsSpace(r) || r == '-' || r == '(' || r == '{' || r == '[' || r == '<'
}

// FilterTrim strips leading and trailing characters from a string.
//
// By default, it strips whitespace characters. You can optionally provide
// a string of characters to trim as the first argument.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("trim", FilterTrim)
//
// Template usage:
//
//	{{ "  hello  "|trim }}
//	  -> "hello"
//	{{ "xxxhelloxxx"|trim("x") }}
//	  -> "hello"
func FilterTrim(state State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	// Like in Rust, a chars argument that is not a string is converted to its
	// string representation, while none and undefined trim whitespace.
	if len(args) > 0 && !args[0].IsNone() && !args[0].IsUndefined() {
		chars, ok := args[0].AsString()
		if !ok {
			chars = args[0].String()
		}
		return preserveSafety(val, strings.Trim(s, chars)), nil
	}
	// Like str::trim in Rust, strip all Unicode whitespace by default
	return preserveSafety(val, strings.TrimSpace(s)), nil
}

// FilterReplace replaces occurrences of a substring with another string.
//
// This filter replaces all occurrences of the first parameter with the second.
// Optionally, you can provide a third parameter to limit the number of replacements.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("replace", FilterReplace)
//
// Template usage:
//
//	{{ "Hello World"|replace("Hello", "Goodbye") }}
//	  -> "Goodbye World"
//	{{ "aaa"|replace("a", "b", 2) }}
//	  -> "bba"
func FilterReplace(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if len(args) < 2 {
		return val, fmt.Errorf("replace requires old and new arguments")
	}
	if len(args) > 3 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many arguments")
	}
	countArg, hasCount := kwargs["count"]
	for name := range kwargs {
		if name != "count" || len(args) > 2 {
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, fmt.Sprintf("unknown keyword argument '%s'", name))
		}
	}
	if len(args) > 2 {
		countArg, hasCount = args[2], true
	}
	// Like in Rust and Python, a missing or negative count replaces all
	// occurrences.
	count := -1
	if hasCount && !countArg.IsNone() && !countArg.IsUndefined() {
		c, err := replaceCount(countArg)
		if err != nil {
			return value.Undefined(), err
		}
		count = c
	}
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	old, err := stringInput(state, args[0])
	if err != nil {
		return value.Undefined(), err
	}
	new, err := stringInput(state, args[1])
	if err != nil {
		return value.Undefined(), err
	}

	safetyAware := autoEscapeEnabled(state) && (val.IsSafe() || args[0].IsSafe() || args[1].IsSafe())
	if safetyAware {
		s, err = formatForSafeOutput(state, val, false)
		if err != nil {
			return value.Undefined(), err
		}
		new, err = formatForSafeOutput(state, args[1], false)
		if err != nil {
			return value.Undefined(), err
		}
		if err := ensureFuelForReplace(state, s, old, new, count); err != nil {
			return value.Undefined(), err
		}
		return value.FromSafeString(strings.Replace(s, old, new, count)), nil
	}
	if err := ensureFuelForReplace(state, s, old, new, count); err != nil {
		return value.Undefined(), err
	}
	return value.FromString(strings.Replace(s, old, new, count)), nil
}

// usizeArg converts an argument like a usize argument in Rust: booleans count
// as 0 and 1, floats must be integral and negative or other values fail.
func usizeArg(v value.Value) (int, error) {
	if b, ok := v.AsBool(); ok {
		if b {
			return 1, nil
		}
		return 0, nil
	}
	if f, ok := v.AsFloat(); ok && f == math.Trunc(f) && f >= 0 && f < math.MaxInt64 {
		if n, ok := v.AsInt(); ok {
			return int(n), nil
		}
	}
	return 0, mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("cannot convert %s to usize", v.Kind()))
}

// replaceCount converts the count argument of replace like an i64 argument
// in Rust: booleans count as 0 and 1, floats must be integral and other
// values fail.
func replaceCount(v value.Value) (int, error) {
	if b, ok := v.AsBool(); ok {
		if b {
			return 1, nil
		}
		return 0, nil
	}
	if f, ok := v.AsFloat(); ok && f == math.Trunc(f) && f >= math.MinInt64 && f < math.MaxInt64 {
		if c, ok := v.AsInt(); ok {
			if c < 0 {
				return -1, nil
			}
			return int(min(c, math.MaxInt)), nil
		}
	}
	return 0, mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("cannot convert %s to i64", v.Kind()))
}

// ensureFuelForReplace refuses a replacement that would create a string
// larger than the remaining fuel can pay for.
func ensureFuelForReplace(state State, s, old, new string, count int) error {
	if len(new) <= len(old) || fuelOf(state) == nil {
		return nil
	}
	size := func(matches int) int {
		return saturatingAdd(saturatingMul(matches, len(new)-len(old)), len(s))
	}
	// the cheap upper bound assumes that everything matches, only if that
	// cannot be paid for are the matches counted.
	maxMatches := len(s)/max(len(old), 1) + 1
	if count >= 0 {
		maxMatches = min(maxMatches, count)
	}
	if ensureBytesQuiet(state, size(maxMatches)) != nil {
		matches := strings.Count(s, old)
		if count >= 0 {
			matches = min(matches, count)
		}
		return ensureBytes(state, size(matches))
	}
	return nil
}

// FilterFormat applies printf-style formatting to a string.
//
// Keyword arguments are passed as a trailing mapping, so they can fill
// %(name)s specs.
//
// Example:
//
//	{{ "%s, %s!"|format(greeting, name) }}
//	{{ "%(greeting)s, %(name)s!"|format(greeting=greeting, name=name) }}
func FilterFormat(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	formatStr, ok := val.AsString()
	if !ok {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "format filter expects a string")
	}
	if len(kwargs) > 0 {
		args = append(args[:len(args):len(args)], value.FromMap(kwargs))
	}

	// like any other string conversion, formatting undefined values fails
	// with strict undefined behavior.
	behavior := undefinedBehavior(state)
	strict := behavior == value.UndefinedStrict || behavior == value.UndefinedSemiStrict
	isSafe := val.IsSafe()
	// width and precision can request arbitrarily large output, so check
	// that the fuel can pay for it before formatting.
	minOutputLen := len(formatStr)
	transform := func(arg value.Value, verb byte, minLen int) (value.Value, error) {
		minOutputLen = saturatingAdd(minOutputLen, minLen)
		if err := chargeFormatSpec(state); err != nil {
			return value.Undefined(), err
		}
		if err := ensureBytes(state, minOutputLen); err != nil {
			return value.Undefined(), err
		}
		if strict && arg.IsUndefined() && !arg.IsSilentUndefined() {
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrUndefinedVar, "undefined value")
		}
		if isSafe {
			if verb == 'c' {
				return value.Undefined(), mjerrors.NewError(
					mjerrors.ErrInvalidOperation,
					"character formatting is not supported for safe format strings",
				)
			}
			if arg.IsSafe() || arg.Kind() == value.KindBool || arg.Kind() == value.KindNumber {
				return arg, nil
			}
			escaped, err := formatForSafeOutput(state, arg, true)
			if err != nil {
				return value.Undefined(), err
			}
			return value.FromString(escaped), nil
		}
		return arg, nil
	}

	formatted, err := formatPrintfWith(formatStr, args, transform)
	if err != nil {
		return value.Undefined(), err
	}
	if val.IsSafe() {
		return value.FromSafeString(formatted), nil
	}
	return value.FromString(formatted), nil
}

func formatPrintf(formatStr string, args []value.Value) (string, error) {
	return formatPrintfWith(formatStr, args, nil)
}

// formatPrintfWith formats like printf and calls transform for every value.
//
// The transform callback is also given the minimum number of bytes the
// formatted value will need (as requested by width and precision) so that
// the caller can refuse huge allocations.
func formatPrintfWith(formatStr string, args []value.Value, transform func(value.Value, byte, int) (value.Value, error)) (string, error) {
	var out strings.Builder
	argIndex := 0

	for i := 0; i < len(formatStr); {
		if formatStr[i] != '%' {
			out.WriteByte(formatStr[i])
			i++
			continue
		}
		if i+1 < len(formatStr) && formatStr[i+1] == '%' {
			out.WriteByte('%')
			i += 2
			continue
		}

		location := i
		i++

		key := ""
		if i < len(formatStr) && formatStr[i] == '(' {
			end := strings.IndexByte(formatStr[i+1:], ')')
			if end < 0 {
				return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, "incomplete format key")
			}
			key = formatStr[i+1 : i+1+end]
			i += end + 2
		}

		flagsStart := i
		for i < len(formatStr) && strings.ContainsRune("#0- +", rune(formatStr[i])) {
			i++
		}
		flags := formatStr[flagsStart:i]

		widthStart := i
		for i < len(formatStr) && formatStr[i] >= '0' && formatStr[i] <= '9' {
			i++
		}
		width := formatStr[widthStart:i]

		precision := ""
		if i < len(formatStr) && formatStr[i] == '.' {
			i++
			precStart := i
			for i < len(formatStr) && formatStr[i] >= '0' && formatStr[i] <= '9' {
				i++
			}
			precision = formatStr[precStart:i]
		}

		if i < len(formatStr) && (formatStr[i] == 'h' || formatStr[i] == 'l' || formatStr[i] == 'L') {
			i++
		}

		if i >= len(formatStr) {
			return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, "incomplete format specifier")
		}
		verb := formatStr[i]
		i++

		if !strings.ContainsRune("cdiouxXeEfFgGs", rune(verb)) {
			return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("invalid format verb '%c'", verb))
		}

		var arg value.Value
		if key != "" {
			if len(args) == 0 {
				return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("missing an argument for format spec at offset %d", location))
			}
			mapping, ok := args[0].AsValueMap()
			if !ok {
				return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, "format argument must be a mapping")
			}
			mapped, ok := mapping.Get(value.FromString(key))
			if !ok || mapped.IsUndefined() {
				return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("missing an argument for format spec at offset %d", location))
			}
			arg = mapped
		} else {
			if argIndex >= len(args) {
				return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("missing an argument for format spec at offset %d", location))
			}
			arg = args[argIndex]
			argIndex++
		}

		if transform != nil {
			var err error
			arg, err = transform(arg, verb, saturatingAdd(parseFormatNumber(width), parseFormatNumber(precision)))
			if err != nil {
				return "", err
			}
		}

		formatted, err := formatPrintfValue(arg, flags, width, precision, verb, location)
		if err != nil {
			return "", err
		}
		out.WriteString(formatted)
	}

	return out.String(), nil
}

// parseFormatNumber parses the width or precision of a format spec.  Numbers
// that are too large saturate.
func parseFormatNumber(s string) int {
	n := 0
	for i := 0; i < len(s); i++ {
		n = saturatingAdd(saturatingMul(n, 10), int(s[i]-'0'))
	}
	return n
}

func formatPrintfValue(val value.Value, flags, width, precision string, verb byte, location int) (string, error) {
	format := "%" + flags + width
	if precision != "" {
		format += "." + precision
	}
	format += string(verb)

	switch verb {
	case 's':
		if val.IsActualInt() {
			return fmt.Sprintf(format, val.String()), nil
		}
		// Like in Python, only integers are zero padded.
		format = "%" + strings.ReplaceAll(flags, "0", "") + width
		if precision != "" {
			format += "." + precision
		}
		text := val.String()
		if f, ok := val.AsFloat(); ok && val.IsActualFloat() {
			text = pythonFloatString(f)
		}
		return fmt.Sprintf(format+"s", text), nil
	case 'c':
		var char rune
		switch {
		case val.IsActualInt():
			i, _ := val.AsInt()
			if i < 0 || i > unicode.MaxRune || i >= 0xd800 && i <= 0xdfff {
				return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, "character format ('c') arg not in range(0x110000)")
			}
			char = rune(i)
		case val.Kind() == value.KindBool:
			b, _ := val.AsBool()
			if b {
				char = 1
			}
		case val.Kind() == value.KindString:
			s, _ := val.AsString()
			runes := []rune(s)
			if len(runes) != 1 {
				return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, "character format ('c') requires integer or char")
			}
			char = runes[0]
		default:
			return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, "character format ('c') requires integer or char")
		}
		charStr := string(char)
		widthValue, _ := strconv.Atoi(width)
		padding := strings.Repeat(" ", max(0, widthValue-1))
		if strings.Contains(flags, "-") {
			return charStr + padding, nil
		}
		return padding + charStr, nil
	case 'd', 'i', 'o', 'x', 'X':
		if i, ok := val.AsInt(); ok {
			return fmt.Sprintf(format, i), nil
		}
		return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("invalid format spec at offset %d", location))
	case 'e', 'E', 'f', 'F', 'g', 'G':
		if f, ok := val.AsFloat(); ok {
			return fmt.Sprintf(format, f), nil
		}
		if i, ok := val.AsInt(); ok {
			return fmt.Sprintf(format, float64(i)), nil
		}
		return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("invalid format spec at offset %d", location))
	default:
		return "", mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("invalid format spec at offset %d", location))
	}
}

// pythonFloatString formats a float like Python's str() which is what
// printf-style formatting uses for '%s'.  Unlike the regular float display,
// this switches to exponent notation outside of 1e-4 <= |f| < 1e16.
func pythonFloatString(f float64) string {
	switch {
	case math.IsNaN(f):
		return "nan"
	case math.IsInf(f, 1):
		return "inf"
	case math.IsInf(f, -1):
		return "-inf"
	}
	if abs := math.Abs(f); abs != 0 && (abs < 1e-4 || abs >= 1e16) {
		return strconv.FormatFloat(f, 'e', -1, 64)
	}
	s := strconv.FormatFloat(f, 'f', -1, 64)
	if !strings.Contains(s, ".") {
		s += ".0"
	}
	return s
}

// FilterDefault provides a default value if the input is undefined.
//
// If the value is undefined, it returns the provided default value.
// Setting the optional second parameter to true will also treat empty/falsy
// values as undefined.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("default", FilterDefault)
//
// Template usage:
//
//	{{ my_variable|default("default value") }}
//	{{ ""|default("empty", true) }}
//	  -> "empty"
func FilterDefault(_ State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if len(kwargs) > 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many keyword arguments")
	}

	if val.IsUndefined() {
		if len(args) > 0 {
			return args[0], nil
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("safe", FilterSafe)
//
// Template usage:
//
//	{{ html_content|safe }}
//
// Warning: Only use this filter on values you trust to contain safe HTML.
// Using it on untrusted content can lead to security vulnerabilities.
func FilterSafe(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	return value.FromSafeString(s), nil
}

// FilterEscape escapes a string for safe output.
//
// By default, this filter is also registered under the alias "e". If the value
// is already marked as safe, it is returned unchanged. Otherwise, it uses the
// current auto-escape mode, falling back to HTML.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("escape", FilterEscape)
//
// Template usage:
//
//	{{ user_input|escape }}
//	{{ "<script>alert('xss')</script>"|e }}
//	  -> "&lt;script&gt;alert('xss')&lt;/script&gt;"
func FilterEscape(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsSafe() {
		return val, nil
	}
	escaped, err := formatForSafeOutput(state, val, true)
	if err != nil {
		return value.Undefined(), err
	}
	return value.FromSafeString(escaped), nil
}

// EscapeHTML escapes a string for safe use in HTML.
func EscapeHTML(s string) string {
	if strings.IndexAny(s, "<>&\"'/") < 0 && utf8.ValidString(s) {
		return s
	}
	var b strings.Builder
	b.Grow(len(s) + len(s)/8)
	last := 0
	for i := 0; i < len(s); i++ {
		var escaped string
		switch s[i] {
		case '<':
			escaped = "&lt;"
		case '>':
			escaped = "&gt;"
		case '&':
			escaped = "&amp;"
		case '"':
			escaped = "&quot;"
		case '\'':
			escaped = "&#x27;"
		case '/':
			escaped = "&#x2f;"
		default:
			continue
		}
		writeValidUTF8(&b, s[last:i])
		b.WriteString(escaped)
		last = i + 1
	}
	writeValidUTF8(&b, s[last:])
	return b.String()
}

// writeValidUTF8 writes a string and replaces invalid bytes with U+FFFD.
func writeValidUTF8(b *strings.Builder, s string) {
	if utf8.ValidString(s) {
		b.WriteString(s)
		return
	}
	for _, r := range s {
		b.WriteRune(r)
	}
}

// FilterString converts a value into a string if it's not one already.
//
// If the value is already a string (and marked as safe if applicable),
// that value is preserved. Otherwise, the value is converted to its
// string representation.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("string", FilterString)
//
// Template usage:
//
//	{{ 42|string }}
//	  -> "42"
func FilterString(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if err := assertNotUndefined(state, val); err != nil {
		return value.Undefined(), err
	}
	if val.Kind() == value.KindString {
		return val, nil
	}
	rv := val.String()
	if isContainer(val) {
		if err := chargeSerialized(state, len(rv)); err != nil {
			return value.Undefined(), err
		}
	}
	return value.FromString(rv), nil
}

// isContainer returns true for values that are converted to strings by
// walking them.
func isContainer(val value.Value) bool {
	switch val.Kind() {
	case value.KindSeq, value.KindMap, value.KindIterable, value.KindPlain:
		return true
	}
	return false
}

// FilterBool converts a value into a boolean.
//
// This filter evaluates the truthiness of a value according to MiniJinja's
// rules: non-zero numbers, non-empty strings, and non-empty collections
// are true.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("bool", FilterBool)
//
// Template usage:
//
//	{{ 42|bool }}
//	  -> true
//	{{ ""|bool }}
//	  -> false
func FilterBool(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	behavior := undefinedBehavior(state)
	if val.IsUndefined() && behavior == value.UndefinedStrict && !val.IsSilentUndefined() {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrUndefinedVar, "undefined value")
	}
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
//	env := minijinja.NewEnvironment()
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
func FilterSplit(_ State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringOrBytesInput(val)
	if err != nil {
		return value.Undefined(), err
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
		result[i] = preserveSafety(val, p)
	}
	return value.FromSlice(result), nil
}

func splitWhitespaceN(s string, n int) []string {
	// Like Python's str.split(None, n - 1): at most n - 1 splits, leading
	// whitespace is skipped and the rest keeps its trailing whitespace.
	var result []string
	for len(result) < n-1 {
		s = strings.TrimLeftFunc(s, unicode.IsSpace)
		if s == "" {
			return result
		}
		i := strings.IndexFunc(s, unicode.IsSpace)
		if i < 0 {
			return append(result, s)
		}
		result = append(result, s[:i])
		s = s[i:]
	}
	if s = strings.TrimLeftFunc(s, unicode.IsSpace); s != "" {
		result = append(result, s)
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("lines", FilterLines)
//
// Template usage:
//
//	{{ "foo\nbar\nbaz"|lines }}
//	  -> ["foo", "bar", "baz"]
func FilterLines(_ State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	s, err := stringOrBytesInput(val)
	if err != nil {
		return value.Undefined(), err
	}

	var lines []string
	if s != "" {
		lines = strings.Split(s, "\n")
		if strings.HasSuffix(s, "\n") {
			lines = lines[:len(lines)-1]
		}
		for i := 0; i < len(lines)-1; i++ {
			lines[i] = strings.TrimSuffix(lines[i], "\r")
		}
		if strings.HasSuffix(s, "\r\n") && len(lines) > 0 {
			lines[len(lines)-1] = strings.TrimSuffix(lines[len(lines)-1], "\r")
		}
	}

	result := make([]value.Value, len(lines))
	for i, line := range lines {
		result[i] = preserveSafety(val, line)
	}
	return value.FromSlice(result), nil
}

// FilterLength returns the number of items in a collection or string.
//
// This filter works on sequences, maps, and strings. For strings, it returns
// the number of characters. Like in Jinja2, undefined values have a length of
// 0 unless strict undefined behavior is used. This filter is also commonly
// available under the alias "count".
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("length", FilterLength)
//
// Template usage:
//
//	<p>{{ users|length }} users found</p>
//	{{ "hello"|length }}
//	  -> 5
func FilterLength(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() {
		// like in Jinja2 undefined has a length of 0
		if err := assertNotUndefined(state, val); err != nil {
			return value.Undefined(), err
		}
		return value.FromInt(0), nil
	}
	if l, ok := val.Len(); ok {
		return value.FromInt(int64(l)), nil
	}
	return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation,
		fmt.Sprintf("cannot calculate length of value of type %s", val.Kind()))
}

// FilterFirst returns the first item from an iterable.
//
// If the iterable is empty, undefined is returned. Like in Jinja2, undefined
// values are returned unchanged unless strict undefined behavior is used.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("first", FilterFirst)
//
// Template usage:
//
//	<dl>
//	  <dt>primary email
//	  <dd>{{ user.email_addresses|first|default('no email') }}
//	</dl>
func FilterFirst(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() {
		if err := assertNotUndefined(state, val); err != nil {
			return value.Undefined(), err
		}
		return val, nil
	}
	// host objects are iterated lazily so that only the first item is
	// produced.
	if _, isIterable := val.Raw().(value.Iterable); !isIterable {
		if obj, ok := val.AsObject(); ok {
			if seq := value.IterateObject(obj); seq != nil {
				for item := range seq {
					return item, nil
				}
				return value.Undefined(), nil
			}
		}
	}
	items := val.Iter()
	if items == nil {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot get first item from value")
	}
	if len(items) > 0 {
		return items[0], nil
	}
	return value.Undefined(), nil
}

// FilterLast returns the last item from an iterable.
//
// If the iterable is empty, undefined is returned. Like in Jinja2, undefined
// values are returned unchanged unless strict undefined behavior is used.
//
// Example:
//
//	env := minijinja.NewEnvironment()
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
func FilterLast(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() {
		if err := assertNotUndefined(state, val); err != nil {
			return value.Undefined(), err
		}
		return val, nil
	}
	if s, ok := val.AsString(); ok {
		runes := []rune(s)
		if len(runes) > 0 {
			return preserveSafety(val, string(runes[len(runes)-1])), nil
		}
		return value.Undefined(), nil
	}
	if kind := val.Kind(); kind != value.KindSeq && kind != value.KindIterable && kind != value.KindMap {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot get last item from value")
	}
	items, err := lastItems(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	if len(items) > 0 {
		return items[len(items)-1], nil
	}
	return value.Undefined(), nil
}

// lastItems returns items of which the last one is the last item of the
// value.  Sequences are indexed directly while iterables need to be consumed,
// which is charged per item.
func lastItems(state State, val value.Value) ([]value.Value, error) {
	if fuel.IsMaterialized(val) {
		return val.Iter(), nil
	}
	obj, _ := val.AsObject()
	if seq, ok := obj.(value.SeqObject); ok && value.GetObjectRepr(obj) == value.ObjectReprSeq {
		if n := seq.SeqLen(); n > 0 {
			return []value.Value{seq.SeqItem(n - 1)}, nil
		}
		return nil, nil
	}
	return fuelItems(state, val)
}

// FilterReverse reverses an iterable or string.
//
// For strings, this reverses the characters. For iterables, it reverses
// the order of items. Like in Jinja2, none cannot be reversed. Undefined
// values are returned unchanged unless strict undefined behavior is used.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("reverse", FilterReverse)
//
// Template usage:
//
//	{% for user in users|reverse %}
//	  <li>{{ user.name }}
//	{% endfor %}
//	{{ "hello"|reverse }}
//	  -> "olleh"
func FilterReverse(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() {
		if err := assertNotUndefined(state, val); err != nil {
			return value.Undefined(), err
		}
		return val, nil
	}
	if s, ok := val.AsString(); ok {
		runes := []rune(s)
		for i, j := 0, len(runes)-1; i < j; i, j = i+1, j-1 {
			runes[i], runes[j] = runes[j], runes[i]
		}
		return preserveSafety(val, string(runes)), nil
	}
	if bytes, ok := val.Raw().([]byte); ok {
		result := append([]byte(nil), bytes...)
		for i, j := 0, len(result)-1; i < j; i, j = i+1, j-1 {
			result[i], result[j] = result[j], result[i]
		}
		return value.FromBytes(result), nil
	}

	// host objects need to be consumed, which is charged per item.  The
	// reversed copy is charged as the result of the filter.
	var items []value.Value
	if fuel.IsMaterialized(val) {
		items = val.Iter()
	} else {
		var err error
		if items, err = fuelItems(state, val); err != nil {
			return value.Undefined(), err
		}
	}
	if items != nil {
		if err := ensureSlots(state, len(items)); err != nil {
			return value.Undefined(), err
		}
		result := make([]value.Value, len(items))
		for i, item := range items {
			result[len(items)-1-i] = item
		}
		return value.FromIterator(value.NewIterator("reversed", result)), nil
	}
	return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation,
		fmt.Sprintf("cannot reverse values of type %s", val.Kind()))
}

// FilterSort sorts an iterable.
//
// The filter accepts several keyword arguments to control sorting behavior:
//
//   - reverse: set to true to sort in descending order
//   - case_sensitive: set to true for case-sensitive string sorting (default: false)
//   - attribute: can be set to an attribute name or dotted path to sort by that attribute,
//     or to a comma-separated list of them to sort by a composite key like "age, name"
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("sort", FilterSort)
//
// Template usage:
//
//	{{ [3, 1, 2]|sort }}
//	  -> [1, 2, 3]
//	{{ users|sort(attribute="age") }}
//	{{ users|sort(attribute="age", reverse=true) }}
//	{{ cities|sort(attribute="name, state") }}
func FilterSort(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
	if items == nil {
		return val, nil
	}
	if err := chargeSort(state, len(items)); err != nil {
		return value.Undefined(), err
	}

	if len(args) > 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many arguments")
	}

	reverse := false
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

	// Get attributes for sorting.  Like in Rust, a comma-separated list
	// sorts by a composite key of all the attributes.
	var attrNames []string
	if attr, ok := kwargs["attribute"]; ok {
		if s, ok := attr.AsString(); ok {
			for _, name := range strings.Split(s, ",") {
				if name = strings.TrimSpace(name); name != "" {
					attrNames = append(attrNames, name)
				}
			}
		}
	}

	// Like in Jinja2, the parts of a composite key are compared case
	// insensitively unless case_sensitive is set.
	sortKey := func(item value.Value) value.Value {
		switch len(attrNames) {
		case 0:
			return item
		case 1:
			return getDeepAttr(item, attrNames[0])
		}
		key := make([]value.Value, len(attrNames))
		for i, name := range attrNames {
			key[i] = getDeepAttr(item, name)
			if s, ok := key[i].AsString(); ok && !caseSensitive {
				key[i] = value.FromString(strings.ToLower(s))
			}
		}
		return value.FromSlice(key)
	}

	type keyedItem struct{ key, item value.Value }
	keyed := make([]keyedItem, len(items))
	for i, item := range items {
		keyed[i] = keyedItem{sortKey(item), item}
	}

	sort.SliceStable(keyed, func(i, j int) bool {
		a, b := keyed[i].key, keyed[j].key

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

	result := make([]value.Value, len(keyed))
	for i, k := range keyed {
		result[i] = k.item
	}
	return value.FromSlice(result), nil
}

// getDeepAttr gets a nested attribute (supports "a.b.0" syntax)
func getDeepAttr(v value.Value, path string) value.Value {
	if !strings.Contains(path, ".") && !mayBeInt(path) {
		return v.GetAttr(path)
	}
	parts := strings.Split(path, ".")
	for _, part := range parts {
		// Try as integer index first
		idx, err := int64(0), errNotInt
		if mayBeInt(part) {
			idx, err = parseInt(part)
		}
		if err == nil {
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

var errNotInt = fmt.Errorf("not an integer")

// mayBeInt returns false for path segments that parseInt certainly rejects.
func mayBeInt(s string) bool {
	if s == "" {
		return true
	}
	c := s[0]
	return !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c == '_')
}

func parseInt(s string) (int64, error) {
	var n int64
	_, err := fmt.Sscanf(s, "%d", &n)
	return n, err
}

// FilterJoin concatenates items from an iterable into a string.
//
// Parameters, passed positionally or by keyword:
//   - d: the separator string to use between items (default: no separator)
//   - attribute: join this attribute or dotted path of the items instead
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("join", FilterJoin)
//
// Template usage:
//
//	{{ ["a", "b", "c"]|join(", ") }}
//	  -> "a, b, c"
//	{{ items|join }}
//	{{ users|join(", ", attribute="username") }}
func FilterJoin(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	params, err := filterParams(args, kwargs, "d", "attribute")
	if err != nil {
		return value.Undefined(), err
	}
	if err := assertNotUndefined(state, val); err != nil {
		return value.Undefined(), err
	}
	if val.IsNone() || val.IsUndefined() {
		val = value.FromSlice(nil)
	}
	items, err := fuelOf(state).Items(val)
	if err != nil {
		return value.Undefined(), err
	}
	if items == nil {
		return val, nil
	}
	items, err = mapAttribute(items, params)
	if err != nil {
		return value.Undefined(), err
	}

	sepVal, hasSep := params["d"]
	sep := ""
	sepSafe := false
	if hasSep {
		sep = sepVal.String()
		sepSafe = sepVal.IsSafe()
	}

	outputSafe := false
	if autoEscapeEnabled(state) {
		outputSafe = sepSafe
		if !outputSafe {
			for _, item := range items {
				if item.IsSafe() {
					outputSafe = true
					break
				}
			}
		}
	}
	if !outputSafe {
		rv, err := joinItems(state, items, sep, func(item value.Value) (string, error) {
			return item.String(), nil
		})
		if err != nil {
			return value.Undefined(), err
		}
		return value.FromString(rv), nil
	}

	if !sepSafe && hasSep {
		sep, err = formatForSafeOutput(state, sepVal, false)
		if err != nil {
			return value.Undefined(), err
		}
	}
	rv, err := joinItems(state, items, sep, func(item value.Value) (string, error) {
		return formatForSafeOutput(state, item, false)
	})
	if err != nil {
		return value.Undefined(), err
	}
	return value.FromSafeString(rv), nil
}

// joinItems joins the formatted items with a separator.
//
// The separator is repeated for every item, so the size of the result is not
// bounded by the size of the items.  The size of the result is checked
// against the fuel before it is built.
func joinItems(state State, items []value.Value, sep string, format func(value.Value) (string, error)) (string, error) {
	parts := make([]string, len(items))
	size := 0
	for i, item := range items {
		str, err := format(item)
		if err != nil {
			return "", err
		}
		parts[i] = str
		size = saturatingAdd(size, len(str))
	}
	if sep != "" && len(parts) > 1 && fuelOf(state) != nil {
		size = saturatingAdd(size, saturatingMul(len(sep), len(parts)-1))
		if err := ensureBytes(state, size); err != nil {
			return "", err
		}
	}
	return strings.Join(parts, sep), nil
}

// FilterList converts a value into a list.
//
// If the value is already a list, it's returned unchanged. For maps, this
// returns a list of keys. For strings, this returns the characters. If the
// value is undefined, an empty list is returned.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("list", FilterList)
//
// Template usage:
//
//	{{ "abc"|list }}
//	  -> ["a", "b", "c"]
//	{{ range(5)|list }}
//	  -> [0, 1, 2, 3, 4]
func FilterList(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() {
		behavior := undefinedBehavior(state)
		if (behavior == value.UndefinedStrict || behavior == value.UndefinedSemiStrict) && !val.IsSilentUndefined() {
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot convert value to list")
		}
		return value.FromSlice(nil), nil
	}

	items, err := fuelItems(state, val)
	if err != nil {
		return value.Undefined(), err
	}
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("unique", FilterUnique)
//
// Template usage:
//
//	{{ ["a", "b", "a", "c"]|unique }}
//	  -> ["a", "b", "c"]
//	{{ users|unique(attribute="city") }}
func FilterUnique(state State, val value.Value, _ []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
	if items == nil {
		return val, nil
	}

	caseSensitive := false
	if cs, ok := kwargs["case_sensitive"]; ok {
		if b, ok := cs.AsBool(); ok {
			caseSensitive = b
		}
	}

	attrName := ""
	if attr, ok := kwargs["attribute"]; ok {
		if s, ok := attr.AsString(); ok {
			attrName = s
		} else {
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "attribute must be a string")
		}
	}

	seen := make(map[string]bool)
	var result []value.Value
	for _, item := range items {
		valueToCompare := item
		if attrName != "" {
			valueToCompare = getDeepAttr(item, attrName)
		}

		key := uniqueKey(valueToCompare, caseSensitive)
		if err := chargeLookup(state, len(seen)); err != nil {
			return value.Undefined(), err
		}
		if !seen[key] {
			seen[key] = true
			result = append(result, item)
		}
	}
	return value.FromSlice(result), nil
}

// uniqueKey returns the key under which unique detects duplicates.  Like in
// Rust, booleans and numbers that compare equal (such as true, 1 and 1.0) are
// the same item, also inside of sequences, while strings never collide with
// other values.
func uniqueKey(v value.Value, caseSensitive bool) string {
	if s, ok := v.AsString(); ok {
		if !caseSensitive {
			s = strings.ToLower(s)
		}
		return "s:" + strconv.Quote(s)
	}
	if items, ok := v.AsSlice(); ok {
		var b strings.Builder
		if v.IsTuple() {
			b.WriteString("t:")
		} else {
			b.WriteString("l:")
		}
		for _, item := range items {
			b.WriteString(uniqueKey(item, true))
			b.WriteByte(',')
		}
		return b.String()
	}
	if b, ok := v.AsBool(); ok {
		if b {
			return "n:1"
		}
		return "n:0"
	}
	if v.IsActualFloat() {
		f, _ := v.AsFloat()
		if !math.IsInf(f, 0) && f == math.Trunc(f) {
			i, _ := new(big.Float).SetFloat64(f).Int(nil)
			return "n:" + i.String()
		}
	}
	if v.Kind() == value.KindNumber {
		return "n:" + v.Repr()
	}
	return "v:" + v.Repr()
}

// FilterMin returns the smallest item from an iterable.
//
// If the iterable is empty, undefined is returned.
//
// Keyword arguments:
//   - case_sensitive: set to true to compare strings case-sensitively (default: false)
//   - attribute: compare by this attribute or dotted path; the whole item is returned
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("min", FilterMin)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|min }}
//	  -> 1
//	{{ users|min(attribute="age") }}
func FilterMin(state State, val value.Value, _ []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	return minMax(state, val, kwargs, -1)
}

// FilterMax returns the largest item from an iterable.
//
// If the iterable is empty, undefined is returned.
//
// Keyword arguments:
//   - case_sensitive: set to true to compare strings case-sensitively (default: false)
//   - attribute: compare by this attribute or dotted path; the whole item is returned
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("max", FilterMax)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|max }}
//	  -> 4
//	{{ users|max(attribute="age") }}
func FilterMax(state State, val value.Value, _ []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	return minMax(state, val, kwargs, 1)
}

// minMax returns the smallest or largest item by key (-1 for min, 1 for max).
// The first item is returned when multiple items have equal keys.
func minMax(state State, val value.Value, kwargs map[string]value.Value, want int) (value.Value, error) {
	items, err := fuelOf(state).Items(val)
	if err != nil {
		return value.Undefined(), err
	}
	if len(items) == 0 {
		return value.Undefined(), nil
	}

	caseSensitive := false
	if cs, ok := kwargs["case_sensitive"]; ok {
		if b, ok := cs.AsBool(); ok {
			caseSensitive = b
		}
	}
	var attrName string
	if attr, ok := kwargs["attribute"]; ok {
		if s, ok := attr.AsString(); ok {
			attrName = s
		}
	}

	key := func(v value.Value) value.Value {
		if attrName != "" {
			v = getDeepAttr(v, attrName)
		}
		if !caseSensitive {
			if s, ok := v.AsString(); ok {
				return value.FromString(strings.ToLower(s))
			}
		}
		return v
	}

	best, bestKey := items[0], key(items[0])
	for _, item := range items[1:] {
		k := key(item)
		if cmp, ok := k.Compare(bestKey); ok && ((want < 0 && cmp < 0) || (want > 0 && cmp > 0)) {
			best, bestKey = item, k
		}
	}
	return best, nil
}

// FilterSum sums up all numeric values in an iterable.
//
// Undefined values are skipped.
//
// Parameters, passed positionally or by keyword:
//   - attribute: sum up this attribute or dotted path of the items
//   - start: the value to start from (default: 0)
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("sum", FilterSum)
//
// Template usage:
//
//	{{ [1, 2, 3]|sum }}
//	  -> 6
//	{{ values|sum(start=100) }}
//	  -> sum of values + 100
//	{{ items|sum(attribute="price") }}
func FilterSum(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ft := fuelOf(state)
	params, err := filterParams(args, kwargs, "attribute", "start")
	if err != nil {
		return value.Undefined(), err
	}
	result := value.FromInt(0)
	if start, ok := params["start"]; ok {
		result = start
	}
	items, err := tryFuelItems(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	items, err = mapAttribute(items, params)
	if err != nil {
		return value.Undefined(), err
	}

	for _, item := range items {
		if item.IsUndefined() {
			continue
		}
		result, err = result.Add(item)
		if err != nil {
			return value.Undefined(), err
		}
		// adding up strings or lists creates a new value every time.
		if !result.IsActualInt() && !result.IsActualFloat() {
			if err := chargeValue(ft, result); err != nil {
				return value.Undefined(), err
			}
		}
	}
	return result, nil
}

// mapAttribute replaces the items with their "attribute" parameter, if given.
func mapAttribute(items []value.Value, params map[string]value.Value) ([]value.Value, error) {
	attr, ok := params["attribute"]
	if !ok {
		return items, nil
	}
	path, ok := attr.AsString()
	if !ok {
		return nil, mjerrors.NewError(mjerrors.ErrInvalidOperation, "value is not a string")
	}
	mapped := make([]value.Value, len(items))
	for i, item := range items {
		mapped[i] = getDeepAttr(item, path)
	}
	return mapped, nil
}

// FilterBatch batches items into groups of a given size.
//
// This filter works like FilterSlice but in the other direction. It returns
// a list of lists with the given number of items. If you provide a second
// parameter, it's used to fill up missing items in the last batch.
//
// Example:
//
//	env := minijinja.NewEnvironment()
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
func FilterBatch(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
	if items == nil {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot batch non-iterable")
	}

	if len(kwargs) > 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many keyword arguments")
	}
	if len(args) == 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrMissingArgument, "missing argument")
	}
	if len(args) > 2 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many arguments")
	}
	c, ok := args[0].AsInt()
	if b, isBool := args[0].AsBool(); isBool {
		c, ok = 0, true
		if b {
			c = 1
		}
	}
	if !ok || c < 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "count must be a positive integer")
	}
	if c == 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "count cannot be 0")
	}
	lineCount := int(c)

	// Like in Rust and Jinja2, none counts as no fill value.
	var fillWith *value.Value
	if len(args) > 1 && !args[1].IsNone() && !args[1].IsUndefined() {
		fillWith = &args[1]
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
		if fillWith != nil && len(batch) < lineCount {
			if err := ensureSlots(state, lineCount-len(batch)); err != nil {
				return value.Undefined(), err
			}
			for len(batch) < lineCount {
				batch = append(batch, *fillWith)
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
//	env := minijinja.NewEnvironment()
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
func FilterSlice(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
	if items == nil {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot slice non-iterable")
	}

	if len(kwargs) > 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many keyword arguments")
	}
	if len(args) == 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrMissingArgument, "missing argument")
	}
	if len(args) > 2 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many arguments")
	}
	c, ok := args[0].AsInt()
	if b, isBool := args[0].AsBool(); isBool {
		c, ok = 0, true
		if b {
			c = 1
		}
	}
	if !ok || c < 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "count must be a positive integer")
	}
	if c == 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "count cannot be 0")
	}
	sliceCount := int(c)

	// Like in Rust and Jinja2, none counts as no fill value, and every slice
	// without an extra item gets the fill value appended.
	var fillWith *value.Value
	if len(args) > 1 && !args[1].IsNone() && !args[1].IsUndefined() {
		fillWith = &args[1]
	}

	// every slice is a new list
	if err := ensureSlots(state, saturatingAdd(sliceCount, len(items))); err != nil {
		return value.Undefined(), err
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

		slice := make([]value.Value, size, size+1)
		copy(slice, items[offset:offset+size])

		if fillWith != nil && i >= remainder {
			slice = append(slice, *fillWith)
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
//	env := minijinja.NewEnvironment()
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
func FilterMap(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ft := fuelOf(state)
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
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
			filterFn, ok := state.GetFilter(filterName)
			if !ok {
				return val, fmt.Errorf("unknown filter: %s", filterName)
			}
			if err := chargeCall(ft, item); err != nil {
				return value.Undefined(), err
			}
			var err error
			mapped, err = filterFn(state, item, args[1:], kwargs)
			if err != nil {
				return val, err
			}
			if err := chargeValue(ft, mapped); err != nil {
				return value.Undefined(), err
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("select", FilterSelect)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|select("odd") }}
//	  -> [1, 3]
//	{{ [false, null, 42]|select }}
//	  -> [42]
func FilterSelect(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ft := fuelOf(state)
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
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
			testFn, ok := state.GetTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			if err := chargeCall(ft, item); err != nil {
				return value.Undefined(), err
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("reject", FilterReject)
//
// Template usage:
//
//	{{ [1, 2, 3, 4]|reject("odd") }}
//	  -> [2, 4]
func FilterReject(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ft := fuelOf(state)
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
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
			testFn, ok := state.GetTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			if err := chargeCall(ft, item); err != nil {
				return value.Undefined(), err
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("selectattr", FilterSelectAttr)
//
// Template usage:
//
//	{{ users|selectattr("is_active") }}
//	  -> all users where x.is_active is true
//	{{ users|selectattr("id", "even") }}
//	  -> users with even IDs
func FilterSelectAttr(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ft := fuelOf(state)
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
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
			testFn, ok := state.GetTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			if err := chargeCall(ft, attr); err != nil {
				return value.Undefined(), err
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("rejectattr", FilterRejectAttr)
//
// Template usage:
//
//	{{ users|rejectattr("is_active") }}
//	  -> all users where x.is_active is false
//	{{ users|rejectattr("id", "even") }}
//	  -> users with odd IDs
func FilterRejectAttr(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	ft := fuelOf(state)
	items, iterErr := tryFuelItems(state, val)
	if iterErr != nil {
		return value.Undefined(), iterErr
	}
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
			testFn, ok := state.GetTest(testName)
			if !ok {
				return val, fmt.Errorf("unknown test: %s", testName)
			}
			if err := chargeCall(ft, attr); err != nil {
				return value.Undefined(), err
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
//	env := minijinja.NewEnvironment()
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
func FilterGroupBy(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if err := assertNotUndefined(state, val); err != nil {
		return value.Undefined(), err
	}
	items, err := fuelItems(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	if items == nil {
		return val, nil
	}
	if err := chargeSort(state, len(items)); err != nil {
		return value.Undefined(), err
	}
	if err := ensureSlots(state, saturatingMul(len(items), 2)); err != nil {
		return value.Undefined(), err
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

	// Sort items by group key.  The keys are looked up once up front.
	type keyedItem struct{ key, item value.Value }
	sorted := make([]keyedItem, len(items))
	for i, item := range items {
		sorted[i] = keyedItem{groupByValue(item, attrName, defaultVal), item}
	}

	sort.SliceStable(sorted, func(i, j int) bool {
		return compareGroupBy(sorted[i].key, sorted[j].key, caseSensitive) < 0
	})

	// Group items.  Like in Jinja2 a group is labelled with the key of its
	// first item when keys compare equal without being identical (such as
	// "CA" and "ca" or 1 and 1.0).
	var result []value.Value
	var currentGrouper value.Value
	var currentList []value.Value
	hasGroup := false

	for _, keyed := range sorted {
		item, groupValue := keyed.item, keyed.key
		if hasGroup && groupByEqual(currentGrouper, groupValue, caseSensitive) {
			currentList = append(currentList, item)
			continue
		}
		if hasGroup {
			result = append(result, value.FromObject(&groupObject{
				grouper: currentGrouper,
				list:    currentList,
			}))
		}
		currentGrouper = groupValue
		currentList = []value.Value{item}
		hasGroup = true
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
				// Strings differing only in case compare equal so that the
				// stable sort keeps them in their original order.
				return strings.Compare(strings.ToLower(s1), strings.ToLower(s2))
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

// ObjectRepr makes groups sequences of (grouper, list) like in Rust.
func (g *groupObject) ObjectRepr() value.ObjectRepr {
	return value.ObjectReprSeq
}

func (g *groupObject) SeqLen() int {
	return 2
}

func (g *groupObject) SeqItem(index int) value.Value {
	switch index {
	case 0:
		return g.grouper
	case 1:
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
	if name, ok := key.AsString(); ok {
		return g.GetAttr(name)
	}
	if idx, ok := key.AsInt(); ok {
		if idx < 0 {
			idx += 2
		}
		if idx >= 0 && idx < 2 {
			return g.SeqItem(int(idx))
		}
	}
	return value.Undefined()
}

func (g *groupObject) String() string {
	listRepr := value.FromSlice(g.list).Repr()
	return fmt.Sprintf("(%s, %s)", g.grouper.Repr(), listRepr)
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("chain", FilterChain)
//
// Template usage:
//
//	{{ list1|chain(list2, list3)|length }}
//	{% for user in shard0|chain(shard1, shard2) %}
//	  {{ user.name }}
//	{% endfor %}
func FilterChain(state State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	allValues := append([]value.Value{val}, args...)

	allMaps := true
	allSeq := true
	maps := make([]*value.Map, 0, len(allValues))
	for _, v := range allValues {
		if m, ok := v.AsValueMap(); ok {
			maps = append(maps, m)
		} else {
			allMaps = false
		}
		if _, ok := v.AsSlice(); !ok {
			allSeq = false
		}
	}

	if allMaps {
		// Like in Rust, later maps win and the keys of the merged map are
		// sorted.
		union := value.NewMap()
		for _, m := range maps {
			for k, v := range m.All() {
				union.Set(k, v)
			}
		}
		pairs := make([][2]value.Value, 0, union.Len())
		for k, v := range union.All() {
			pairs = append(pairs, [2]value.Value{k, v})
		}
		sort.SliceStable(pairs, func(i, j int) bool {
			cmp, ok := pairs[i][0].Compare(pairs[j][0])
			return ok && cmp < 0
		})
		merged := value.NewMap()
		for _, pair := range pairs {
			merged.Set(pair[0], pair[1])
		}
		return value.FromValueMap(merged), nil
	}

	// Get items from first value.  Unlike in Rust, chains are materialized,
	// so every item is charged.
	items, err := fuelOf(state).Items(val)
	if err != nil {
		return value.Undefined(), err
	}
	items = append([]value.Value{}, items...)

	// Chain all arguments
	for _, arg := range args {
		argItems, err := fuelOf(state).Items(arg)
		if err != nil {
			return value.Undefined(), err
		}
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

// ObjectRepr makes chains of mixed values iterables like in Rust.
func (c *chainObject) ObjectRepr() value.ObjectRepr {
	return value.ObjectReprIterable
}

func (c *chainObject) Iter() []value.Value {
	return c.items
}

// String renders the chain like other iterables, as in Rust.
func (c *chainObject) String() string {
	return "<iterator>"
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("zip", FilterZip)
//
// Template usage:
//
//	{{ [1, 2, 3]|zip(["a", "b", "c"]) }}
//	  -> [(1, "a"), (2, "b"), (3, "c")]
//	{{ [1, 2]|zip(["a", "b", "c"], ["x", "y", "z"]) }}
//	  -> [(1, "a", "x"), (2, "b", "y")]
func FilterZip(state State, val value.Value, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	// Collect all sequences.  Unlike in Rust, they are materialized, so
	// every item is charged.
	var seqs [][]value.Value
	for _, v := range append([]value.Value{val}, args...) {
		items, err := fuelOf(state).Items(v)
		if err != nil {
			return value.Undefined(), err
		}
		seqs = append(seqs, items)
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
		result[i] = value.FromTuple(tuple)
	}
	return value.FromSlice(result), nil
}

// FilterAbs returns the absolute value of a number.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("abs", FilterAbs)
//
// Template usage:
//
//	{{ -42|abs }}
//	  -> 42
//	{{ 3.14|abs }}
//	  -> 3.14
func FilterAbs(_ State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	switch d := val.Raw().(type) {
	case int64:
		if d == math.MinInt64 {
			return value.FromBigInt(new(big.Int).Neg(big.NewInt(d))), nil
		}
		if d < 0 {
			return value.FromInt(-d), nil
		}
		return val, nil
	case float64:
		return value.FromFloat(math.Abs(d)), nil
	}
	if val.Kind() == value.KindNumber {
		// Big integers.  Like in Rust, they stay within the i128 range, so the
		// smallest i128 has no absolute value.
		if cmp, ok := val.Compare(value.FromBigInt(minI128)); ok && cmp == 0 {
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "overflow on abs")
		}
		if cmp, ok := val.Compare(value.FromInt(0)); ok && cmp < 0 {
			return val.Neg()
		}
		return val, nil
	}
	return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot get absolute value")
}

// FilterInt converts a value to an integer.
//
// String values are parsed as integers in the given base (10 by default).
// For bases 2, 8 and 16 the string may carry a 0b, 0o or 0x prefix. Float
// values are truncated. Boolean true becomes 1, false becomes 0. If
// conversion fails and a default is given, the default is returned, otherwise
// an error. None converts to the default or 0.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("int", FilterInt)
//
// Template usage:
//
//	{{ "42"|int }}
//	  -> 42
//	{{ "0x1f"|int(base=16) }}
//	  -> 31
//	{{ "n/a"|int(default=0) }}
//	  -> 0
func FilterInt(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	params, err := filterParams(args, kwargs, "default", "base")
	if err != nil {
		return value.Undefined(), err
	}
	def, hasDefault := params["default"]
	base := int64(10)
	if b, ok := params["base"]; ok {
		if base, ok = b.AsInt(); !ok {
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "base must be an integer")
		}
	}

	if val.IsUndefined() || val.IsNone() {
		if err := assertNotUndefined(state, val); err != nil {
			return value.Undefined(), err
		}
		if hasDefault {
			return def, nil
		}
		return value.FromInt(0), nil
	}
	if _, ok := asBigInt(val); ok {
		return val, nil
	}
	if val.IsActualFloat() {
		f, _ := val.AsFloat()
		return floatToInt(f), nil
	}
	if i, ok := val.AsInt(); ok {
		return value.FromInt(i), nil
	}
	if b, ok := val.AsBool(); ok {
		if b {
			return value.FromInt(1), nil
		}
		return value.FromInt(0), nil
	}

	var convErr error
	if s, ok := val.AsString(); ok {
		if i, ok := parseIntWithBase(s, base); ok {
			return bigIntValue(i), nil
		}
		f, err := parseFloat(s)
		if err == nil {
			return floatToInt(f), nil
		}
		convErr = mjerrors.NewError(mjerrors.ErrInvalidOperation, err.Error())
	} else {
		convErr = mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("cannot convert %s to integer", val.Kind()))
	}
	if hasDefault {
		return def, nil
	}
	return value.Undefined(), convErr
}

// parseIntWithBase parses an integer like Python's int(s, base) does, except
// that underscores and surrounding whitespace are not accepted.  Like in Rust,
// the result has to fit into an i128.
func parseIntWithBase(s string, base int64) (*big.Int, bool) {
	prefix := ""
	switch {
	case base == 2:
		prefix = "0b"
	case base == 8:
		prefix = "0o"
	case base == 16:
		prefix = "0x"
	case base < 2 || base > 36:
		return nil, false
	}
	sign := ""
	digits := s
	if rest, ok := strings.CutPrefix(digits, "-"); ok {
		sign, digits = "-", rest
	} else {
		digits = strings.TrimPrefix(digits, "+")
	}
	if prefix != "" && len(digits) >= len(prefix) && strings.EqualFold(digits[:len(prefix)], prefix) {
		digits = digits[len(prefix):]
	}
	// SetString accepts underscores only with base 0, but it would take a
	// second sign
	if strings.HasPrefix(digits, "+") || strings.HasPrefix(digits, "-") {
		return nil, false
	}
	i, ok := new(big.Int).SetString(sign+digits, int(base))
	if !ok || !fitsI128(i) {
		return nil, false
	}
	return i, true
}

// FilterFloat converts a value to a float.
//
// String values are parsed as floats. Integer values are converted to floats.
// Boolean true becomes 1.0, false becomes 0.0. If conversion fails and a
// default is given, the default is returned, otherwise an error. None
// converts to the default or 0.0.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("float", FilterFloat)
//
// Template usage:
//
//	{{ "42.5"|float }}
//	  -> 42.5
//	{{ "n/a"|float(default=0.0) }}
//	  -> 0.0
func FilterFloat(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	params, err := filterParams(args, kwargs, "default")
	if err != nil {
		return value.Undefined(), err
	}
	def, hasDefault := params["default"]

	if val.IsUndefined() || val.IsNone() {
		if err := assertNotUndefined(state, val); err != nil {
			return value.Undefined(), err
		}
		if hasDefault {
			return def, nil
		}
		return value.FromFloat(0.0), nil
	}
	if f, ok := val.AsFloat(); ok {
		return value.FromFloat(f), nil
	}
	if n, ok := asBigInt(val); ok {
		f, _ := new(big.Float).SetInt(n).Float64()
		return value.FromFloat(f), nil
	}
	if b, ok := val.AsBool(); ok {
		if b {
			return value.FromFloat(1.0), nil
		}
		return value.FromFloat(0.0), nil
	}

	var convErr error
	if s, ok := val.AsString(); ok {
		f, err := parseFloat(s)
		if err == nil {
			return value.FromFloat(f), nil
		}
		convErr = mjerrors.NewError(mjerrors.ErrInvalidOperation, err.Error())
	} else {
		convErr = mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("cannot convert %s to float", val.Kind()))
	}
	if hasDefault {
		return def, nil
	}
	return value.Undefined(), convErr
}

// filterParams maps the arguments of a filter to their parameter names.
// Each parameter can be passed positionally or by keyword. Like in Rust,
// none and undefined arguments count as not given.
func filterParams(args []value.Value, kwargs map[string]value.Value, names ...string) (map[string]value.Value, error) {
	if len(args) > len(names) {
		return nil, mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many arguments")
	}
	params := make(map[string]value.Value, len(names))
	for i, arg := range args {
		if !arg.IsNone() && !arg.IsUndefined() {
			params[names[i]] = arg
		}
	}
	for name, arg := range kwargs {
		if _, given := params[name]; given || !slices.Contains(names, name) {
			return nil, mjerrors.NewError(mjerrors.ErrTooManyArguments, fmt.Sprintf("unknown keyword argument '%s'", name))
		}
		if !arg.IsNone() && !arg.IsUndefined() {
			params[name] = arg
		}
	}
	return params, nil
}

// assertNotUndefined fails for undefined values with strict and semi-strict
// undefined behavior.
func assertNotUndefined(state State, val value.Value) error {
	behavior := undefinedBehavior(state)
	if (behavior == value.UndefinedStrict || behavior == value.UndefinedSemiStrict) && val.IsUndefined() && !val.IsSilentUndefined() {
		return mjerrors.NewError(mjerrors.ErrUndefinedVar, "undefined value")
	}
	return nil
}

func errRoundTooLarge() error {
	return mjerrors.NewError(mjerrors.ErrInvalidOperation, "rounded value is too large to represent")
}

func roundWithNegativePrecision(f float64, precision int) (float64, error) {
	if math.IsInf(f, 0) || math.IsNaN(f) {
		return f, nil
	}
	if precision <= -309 {
		return math.Copysign(0, f), nil
	}

	decimalPlace := -precision
	// 1074 decimal places are enough to represent any float64 exactly.
	formatted := strconv.FormatFloat(math.Abs(f), 'e', 1074, 64)
	parts := strings.SplitN(formatted, "e", 2)
	exponent, _ := strconv.Atoi(parts[1])
	retainedLen := exponent - decimalPlace + 1
	if retainedLen < 0 {
		return math.Copysign(0, f), nil
	}

	digits := []byte(strings.ReplaceAll(parts[0], ".", ""))
	discarded := digits[retainedLen:]
	lastRetained := byte(0)
	if retainedLen > 0 {
		lastRetained = digits[retainedLen-1] - '0'
	}
	roundUp := discarded[0] > '5'
	if discarded[0] == '5' {
		for _, digit := range discarded[1:] {
			if digit != '0' {
				roundUp = true
				break
			}
		}
		roundUp = roundUp || lastRetained%2 != 0
	}

	retained := append([]byte(nil), digits[:retainedLen]...)
	if roundUp {
		carry := true
		for i := len(retained) - 1; i >= 0; i-- {
			if retained[i] == '9' {
				retained[i] = '0'
			} else {
				retained[i]++
				carry = false
				break
			}
		}
		if carry {
			retained = append([]byte{'1'}, retained...)
		}
	}
	if len(retained) == 0 || strings.Trim(string(retained), "0") == "" {
		return math.Copysign(0, f), nil
	}

	sign := ""
	if math.Signbit(f) {
		sign = "-"
	}
	rounded, err := strconv.ParseFloat(sign+string(retained)+"e"+strconv.Itoa(decimalPlace), 64)
	if err != nil || math.IsInf(rounded, 0) {
		return 0, errRoundTooLarge()
	}
	return rounded, nil
}

// roundDirected rounds a float with ceil or floor like Jinja2 does: the value
// is scaled by 10^precision, rounded to an integer and scaled back.
func roundDirected(f float64, precision int, ceil bool) (float64, error) {
	if math.IsInf(f, 0) || math.IsNaN(f) {
		return f, nil
	}
	factor, _ := strconv.ParseFloat("1e"+strconv.Itoa(precision), 64)
	scaled := f * factor
	if math.IsInf(scaled, 0) {
		// the value has no digits at this precision
		return f, nil
	}
	var rounded float64
	if ceil {
		rounded = math.Ceil(scaled)
	} else {
		rounded = math.Floor(scaled)
	}
	// adding 0.0 turns -0.0 into 0.0 like Python's integer result does
	rv := (rounded + 0) / factor
	if math.IsInf(rv, 0) || math.IsNaN(rv) {
		return 0, errRoundTooLarge()
	}
	return rv, nil
}

// roundInt rounds an integer to a negative precision, that is to a multiple
// of 10^-precision.
func roundInt(i *big.Int, precision int, method string) (value.Value, error) {
	if precision < -38 {
		// like Rust, which rounds in i128: only zero is in reach
		if (method == "ceil" && i.Sign() > 0) || (method == "floor" && i.Sign() < 0) {
			return value.Undefined(), errRoundTooLarge()
		}
		return value.FromInt(0), nil
	}
	factor := new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(-precision)), nil)
	quotient, remainder := new(big.Int).DivMod(i, factor, new(big.Int))
	roundUp := false
	switch method {
	case "ceil":
		roundUp = remainder.Sign() > 0
	case "floor":
	default:
		// round half to even like Python's round()
		cmp := new(big.Int).Lsh(remainder, 1).Cmp(factor)
		roundUp = cmp > 0 || (cmp == 0 && quotient.Bit(0) == 1)
	}
	if roundUp {
		quotient.Add(quotient, big.NewInt(1))
	}
	rv := quotient.Mul(quotient, factor)
	if !fitsI128(rv) {
		return value.Undefined(), errRoundTooLarge()
	}
	return bigIntValue(rv), nil
}

// FilterRound rounds a number to a given precision.
//
// The first parameter specifies the precision (default is 0), the second the
// rounding method: "common" rounds half to even (default), "ceil" always
// rounds up and "floor" always rounds down. Both can also be passed as
// keyword arguments. Integers are returned unchanged unless the precision is
// negative.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("round", FilterRound)
//
// Template usage:
//
//	{{ 42.55|round }}
//	  -> 43.0
//	{{ 42.55|round(1, "floor") }}
//	  -> 42.5
//	{{ 42.15|round(method="ceil") }}
//	  -> 43.0
//	{{ 1234|round(-2) }}
//	  -> 1200
func FilterRound(_ State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	params, err := filterParams(args, kwargs, "precision", "method")
	if err != nil {
		return value.Undefined(), err
	}
	precision := 0
	if p, ok := params["precision"]; ok {
		i, ok := p.AsInt()
		if !ok {
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "precision must be an integer")
		}
		precision = int(i)
	}
	method := "common"
	if m, ok := params["method"]; ok {
		method, _ = m.AsString()
	}
	if method != "common" && method != "ceil" && method != "floor" {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "method must be common, ceil or floor")
	}

	if val.IsActualInt() {
		if precision >= 0 {
			return val, nil
		}
		i, _ := val.AsInt()
		return roundInt(big.NewInt(i), precision, method)
	}
	if n, ok := asBigInt(val); ok {
		if precision >= 0 {
			return val, nil
		}
		if !fitsI128(n) {
			// like Rust, which rounds in i128
			return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot convert number to i128")
		}
		return roundInt(n, precision, method)
	}

	f, ok := val.AsFloat()
	if !ok {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, fmt.Sprintf("cannot round value (%s)", val.Kind()))
	}

	if method != "common" {
		f, err = roundDirected(f, precision, method == "ceil")
		if err != nil {
			return value.Undefined(), err
		}
	} else if precision >= 0 {
		// Decimal formatting uses round-half-even without the intermediate
		// scaling error that would make round(2.675, 2) produce 2.68.
		if precision <= 1074 {
			formatted := strconv.FormatFloat(f, 'f', precision, 64)
			f, _ = strconv.ParseFloat(formatted, 64)
		}
	} else {
		f, err = roundWithNegativePrecision(f, precision)
		if err != nil {
			return value.Undefined(), err
		}
	}

	return value.FromFloat(f), nil
}

// FilterItems returns an iterable of key-value pairs from a map.
//
// This converts a map into a list of [key, value] tuples. The keys of Go maps
// are sorted alphabetically, map objects keep their iteration order. Values
// that are not maps are rejected with an error. Like in Jinja2, undefined
// values give no pairs unless strict undefined behavior is used.
//
// Example:
//
//	env := minijinja.NewEnvironment()
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
func FilterItems(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() {
		// like in Jinja2 undefined has no items
		if err := assertNotUndefined(state, val); err != nil {
			return value.Undefined(), err
		}
		return value.FromSlice(nil), nil
	}
	pairs, ok, err := mapPairs(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	if !ok {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot convert value into pairs")
	}
	result := make([]value.Value, len(pairs))
	for i, pair := range pairs {
		result[i] = value.FromTuple([]value.Value{pair[0], pair[1]})
	}
	return value.FromSlice(result), nil
}

// FilterDictSort sorts a map by keys or values.
//
// Returns a list of [key, value] pairs sorted by key (default) or by value.
//
// Example:
//
//	env := minijinja.NewEnvironment()
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
func FilterDictSort(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	pairs, ok, err := mapPairs(state, val)
	if err != nil {
		return value.Undefined(), err
	}
	if !ok {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, "cannot convert value into pair list")
	}
	if err := chargeSort(state, len(pairs)); err != nil {
		return value.Undefined(), err
	}
	if err := ensureSlots(state, saturatingMul(len(pairs), 3)); err != nil {
		return value.Undefined(), err
	}
	if len(args) > 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many arguments")
	}

	byValue := false
	if b, ok := kwargs["by"]; ok {
		if s, ok := b.AsString(); ok && s == "value" {
			byValue = true
		}
	}

	reverse := false
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

	idx := 0
	if byValue {
		idx = 1
	}
	sort.SliceStable(pairs, func(i, j int) bool {
		cmp := cmpValues(pairs[i][idx], pairs[j][idx])
		if reverse {
			return cmp > 0
		}
		return cmp < 0
	})

	result := make([]value.Value, len(pairs))
	for i, pair := range pairs {
		result[i] = value.FromTuple([]value.Value{pair[0], pair[1]})
	}
	return value.FromSlice(result), nil
}

// FilterAttr looks up an attribute by name.
//
// This is equivalent to using the [] operator in MiniJinja. It's provided
// for compatibility with Jinja2.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("attr", FilterAttr)
//
// Template usage:
//
//	{{ value|attr("key") }}
//	  -> same as value["key"] or value.key
func FilterAttr(_ State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if len(args) < 1 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrMissingArgument, "missing argument")
	}
	if len(args) > 1 || len(kwargs) > 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many arguments")
	}
	// Like Value::get_item in Rust, looking up anything on undefined fails
	// regardless of the undefined behavior.
	if val.IsUndefined() {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrUndefinedVar, "undefined value")
	}
	return val.GetItem(args[0]), nil
}

// FilterIndent indents each line of a string with spaces.
//
// The first parameter sets the indentation width (default: 4). The second
// optional parameter determines whether to indent the first line (default: false).
// The third optional parameter determines whether to indent blank lines (default: false).
//
// Parameters can also be provided as keyword arguments for Jinja2 compatibility:
// width, first, and blank.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("indent", FilterIndent)
//
// Template usage:
//
//	config:
//	{{ yaml_content|indent(2) }}
//	{{ yaml_content|indent(2, true) }}
//	{{ yaml_content|indent(2, true, true) }}
//	{{ yaml_content|indent(width=4, first=true, blank=false) }}
func FilterIndent(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	s, err := stringInput(state, val)
	if err != nil {
		return value.Undefined(), err
	}

	width := 4
	widthArg, hasWidth := value.Undefined(), false
	if len(args) > 0 {
		widthArg, hasWidth = args[0], true
	} else if v, ok := kwargs["width"]; ok {
		widthArg, hasWidth = v, true
		delete(kwargs, "width")
	}
	if hasWidth && !widthArg.IsNone() && !widthArg.IsUndefined() {
		w, err := usizeArg(widthArg)
		if err != nil {
			return value.Undefined(), err
		}
		width = w
	}

	first := false
	if len(args) > 1 {
		if b, ok := args[1].AsBool(); ok {
			first = b
		}
	} else if v, ok := kwargs["first"]; ok {
		if b, ok := v.AsBool(); ok {
			first = b
		}
		delete(kwargs, "first")
	}

	blank := false
	if len(args) > 2 {
		if b, ok := args[2].AsBool(); ok {
			blank = b
		}
	} else if v, ok := kwargs["blank"]; ok {
		if b, ok := v.AsBool(); ok {
			blank = b
		}
		delete(kwargs, "blank")
	}

	if len(kwargs) > 0 {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrTooManyArguments, "too many keyword arguments")
	}

	s = strings.TrimSuffix(s, "\n")
	s = strings.TrimSuffix(s, "\r")
	if fuelOf(state) != nil {
		lines := strings.Count(s, "\n") + 1
		if err := ensureBytes(state, saturatingAdd(saturatingMul(max(width, 0), lines), len(s))); err != nil {
			return value.Undefined(), err
		}
	}
	indent := strings.Repeat(" ", width)
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		if i == 0 && !first {
			continue
		}
		// A blank line in CRLF text is a lone "\r" after splitting on "\n".
		if (line == "" || line == "\r") && !blank {
			continue
		}
		lines[i] = indent + line
	}
	return preserveSafety(val, strings.Join(lines, "\n")), nil
}

// FilterPprint pretty-prints a value for debugging.
//
// This is useful for debugging templates as it formats values in a more
// readable way than the default string representation.
//
// Example:
//
//	env := minijinja.NewEnvironment()
//	env.AddFilter("pprint", FilterPprint)
//
// Template usage:
//
//	<pre>{{ complex_object|pprint }}</pre>
func FilterPprint(state State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	rv := pprintValue(val, 0)
	if err := chargeSerialized(state, len(rv)); err != nil {
		return value.Undefined(), err
	}
	return value.FromString(rv), nil
}

func pprintValue(val value.Value, indent int) string {
	pad := strings.Repeat(" ", indent)
	switch val.Kind() {
	case value.KindSeq, value.KindIterable:
		items := val.Iter()
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
		m, _ := val.AsValueMap()
		if m.Len() == 0 {
			return "{}"
		}
		var sb strings.Builder
		sb.WriteString("{\n")
		for k, v := range m.All() {
			sb.WriteString(strings.Repeat(" ", indent+4))
			sb.WriteString(fmt.Sprintf("%s: %s,", k.Repr(), pprintValue(v, indent+4)))
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
//	env := minijinja.NewEnvironment()
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
func FilterTojson(state State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	// Convert value to Go native type for JSON serialization
	native, err := jsonvalue.ToNative(val)
	if err != nil {
		return value.Undefined(), mjerrors.NewError(mjerrors.ErrInvalidOperation, err.Error())
	}

	// Check for indent option
	indent := ""
	indentArgs := make([]value.Value, 0, 2)
	if len(args) > 0 {
		indentArgs = append(indentArgs, args[0])
	}
	if i, ok := kwargs["indent"]; ok {
		indentArgs = append(indentArgs, i)
	}
	for _, i := range indentArgs {
		if b, ok := i.AsBool(); ok {
			if b {
				indent = "  " // 2 spaces for true
			}
		} else if !i.IsNone() && !i.IsUndefined() {
			n, err := usizeArg(i)
			if err != nil {
				return value.Undefined(), err
			}
			indent = strings.Repeat(" ", n)
		}
	}

	var data []byte
	if indent != "" {
		data, err = json.MarshalIndent(native, "", indent)
	} else {
		data, err = json.Marshal(native)
		data = addCompactJSONSpaces(data)
	}
	if err != nil {
		return value.Undefined(), err
	}
	jsonStr := string(data)
	jsonStr = strings.ReplaceAll(jsonStr, "'", "\\u0027")
	if err := chargeSerialized(state, len(jsonStr)); err != nil {
		return value.Undefined(), err
	}
	return value.FromSafeString(jsonStr), nil
}

func addCompactJSONSpaces(data []byte) []byte {
	result := make([]byte, 0, len(data)+len(data)/8)
	inString := false
	escaped := false
	for _, ch := range data {
		result = append(result, ch)
		if inString {
			if escaped {
				escaped = false
			} else if ch == '\\' {
				escaped = true
			} else if ch == '"' {
				inString = false
			}
			continue
		}
		if ch == '"' {
			inString = true
		} else if ch == ',' || ch == ':' {
			result = append(result, ' ')
		}
	}
	return result
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
//	env := minijinja.NewEnvironment()
//	env.AddFilter("urlencode", FilterUrlencode)
//
// Template usage:
//
//	<a href="/search?{{ {"q": "my search", "lang": "en"}|urlencode }}">
//	{{ "hello world"|urlencode }}
//	  -> "hello%20world"
func FilterUrlencode(_ State, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	// Check if it's a map (dict) - encode as query string
	if m, ok := val.AsValueMap(); ok {
		var parts []string
		for k, v := range m.All() {
			if v.IsNone() {
				continue
			}
			parts = append(parts, urlencodeString(k.String())+"="+urlencodeString(v.String()))
		}
		return value.FromString(strings.Join(parts, "&")), nil
	}

	// Like in Rust, none encodes as an empty string
	if val.IsNone() {
		return value.FromString(""), nil
	}

	s, ok := val.AsString()
	if !ok {
		s = val.String()
	}
	return value.FromString(urlencodeString(s)), nil
}
