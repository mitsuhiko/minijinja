package minijinja

import (
	"fmt"
	"strings"
	"unicode"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// TestDefined checks if a value is defined.
//
// Returns true if the value is not undefined.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("defined", TestDefined)
//
// Template usage:
//
//	{% if my_variable is defined %}
//	  {{ my_variable }}
//	{% endif %}
func TestDefined(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return !val.IsUndefined(), nil
}

// TestUndefined checks if a value is undefined.
//
// Returns true if the value is undefined.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("undefined", TestUndefined)
//
// Template usage:
//
//	{% if my_variable is undefined %}
//	  Variable not set
//	{% endif %}
func TestUndefined(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsUndefined(), nil
}

// TestNone checks if a value is none/null.
//
// Returns true if the value is none.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("none", TestNone)
//
// Template usage:
//
//	{% if value is none %}
//	  Value is null
//	{% endif %}
func TestNone(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsNone(), nil
}

// TestTrue checks if a value is the boolean true.
//
// This is a strict check for the boolean value true, not truthiness.
// Use value.IsTrue() for truthiness checks.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("true", TestTrue)
//
// Template usage:
//
//	{% if value is true %}
//	  Value is exactly true
//	{% endif %}
func TestTrue(_ *State, val value.Value, _ []value.Value) (bool, error) {
	if b, ok := val.AsBool(); ok {
		return b, nil
	}
	return false, nil
}

// TestFalse checks if a value is the boolean false.
//
// This is a strict check for the boolean value false, not falsiness.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("false", TestFalse)
//
// Template usage:
//
//	{% if value is false %}
//	  Value is exactly false
//	{% endif %}
func TestFalse(_ *State, val value.Value, _ []value.Value) (bool, error) {
	if b, ok := val.AsBool(); ok {
		return !b, nil
	}
	return false, nil
}

// TestOdd checks if a number is odd.
//
// Returns true if the value is an odd integer.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("odd", TestOdd)
//
// Template usage:
//
//	{% if loop.index is odd %}
//	  <div class="odd">{{ item }}</div>
//	{% endif %}
//
//	{{ 41 is odd }}
//	  -> true
//	{{ 42 is odd }}
//	  -> false
func TestOdd(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) > 0 {
		return false, NewError(ErrInvalidOperation, "odd test expects no arguments")
	}
	if i, ok := val.AsInt(); ok {
		return i%2 != 0, nil
	}
	return false, nil
}

// TestEven checks if a number is even.
//
// Returns true if the value is an even integer.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("even", TestEven)
//
// Template usage:
//
//	{% for item in items %}
//	  <li class="{{ 'even' if loop.index is even else 'odd' }}">
//	    {{ item }}
//	  </li>
//	{% endfor %}
//
//	{{ 42 is even }}
//	  -> true
//	{{ 41 is even }}
//	  -> false
func TestEven(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) > 0 {
		return false, NewError(ErrInvalidOperation, "even test expects no arguments")
	}
	if i, ok := val.AsInt(); ok {
		return i%2 == 0, nil
	}
	return false, nil
}

// TestDivisibleBy checks if a value is divisible by another number.
//
// Returns true if the value is evenly divisible by the given number.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("divisibleby", TestDivisibleBy)
//
// Template usage:
//
//	{% if count is divisibleby(3) %}
//	  Count is a multiple of 3
//	{% endif %}
//
//	{{ 42 is divisibleby(2) }}
//	  -> true
//	{{ 42 is divisibleby(5) }}
//	  -> false
func TestDivisibleBy(_ *State, val value.Value, args []value.Value) (bool, error) {
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

// TestEq checks if two values are equal.
//
// This is the test version of the == operator. It's useful when combined
// with filters like select/reject.
//
// This test is also registered under the aliases "equalto" and "==".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("eq", TestEq)
//
// Template usage:
//
//	{{ 1 is eq(1) }}
//	  -> true
//	{{ [1, 2, 3]|select("==", 1) }}
//	  -> [1]
func TestEq(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.Equal(args[0]), nil
}

// TestNe checks if two values are not equal.
//
// This is the test version of the != operator. It's useful when combined
// with filters like select/reject.
//
// This test is also registered under the alias "!=".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("ne", TestNe)
//
// Template usage:
//
//	{{ 2 is ne(1) }}
//	  -> true
//	{{ [1, 2, 3]|select("!=", 1) }}
//	  -> [2, 3]
func TestNe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return !val.Equal(args[0]), nil
}

// TestLt checks if a value is less than another.
//
// This is the test version of the < operator. It's useful when combined
// with filters like select/reject.
//
// This test is also registered under the aliases "lessthan" and "<".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("lt", TestLt)
//
// Template usage:
//
//	{{ 1 is lt(2) }}
//	  -> true
//	{{ [1, 2, 3]|select("<", 2) }}
//	  -> [1]
func TestLt(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp < 0, nil
	}
	return false, nil
}

// TestLe checks if a value is less than or equal to another.
//
// This is the test version of the <= operator. It's useful when combined
// with filters like select/reject.
//
// This test is also registered under the alias "<=".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("le", TestLe)
//
// Template usage:
//
//	{{ 1 is le(2) }}
//	  -> true
//	{{ [1, 2, 3]|select("<=", 2) }}
//	  -> [1, 2]
func TestLe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp <= 0, nil
	}
	return false, nil
}

// TestGt checks if a value is greater than another.
//
// This is the test version of the > operator. It's useful when combined
// with filters like select/reject.
//
// This test is also registered under the aliases "greaterthan" and ">".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("gt", TestGt)
//
// Template usage:
//
//	{{ 2 is gt(1) }}
//	  -> true
//	{{ [1, 2, 3]|select(">", 2) }}
//	  -> [3]
func TestGt(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp > 0, nil
	}
	return false, nil
}

// TestGe checks if a value is greater than or equal to another.
//
// This is the test version of the >= operator. It's useful when combined
// with filters like select/reject.
//
// This test is also registered under the alias ">=".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("ge", TestGe)
//
// Template usage:
//
//	{{ 2 is ge(1) }}
//	  -> true
//	{{ [1, 2, 3]|select(">=", 2) }}
//	  -> [2, 3]
func TestGe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp >= 0, nil
	}
	return false, nil
}

// TestIn checks if a value is contained in a sequence.
//
// This is the test version of the "in" operator. It's useful when combined
// with filters like select/reject.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("in", TestIn)
//
// Template usage:
//
//	{{ 1 is in([1, 2, 3]) }}
//	  -> true
//	{{ [1, 2, 3]|select("in", [1, 2]) }}
//	  -> [1, 2]
func TestIn(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return args[0].Contains(val), nil
}

// TestString checks if a value is a string.
//
// Returns true if the value's kind is string.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("string", TestString)
//
// Template usage:
//
//	{{ "42" is string }}
//	  -> true
//	{{ 42 is string }}
//	  -> false
func TestString(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindString, nil
}

// TestNumber checks if a value is a number.
//
// Returns true if the value is a number (either integer or float).
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("number", TestNumber)
//
// Template usage:
//
//	{{ 42 is number }}
//	  -> true
//	{{ "42" is number }}
//	  -> false
func TestNumber(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindNumber, nil
}

// TestInteger checks if a value is an integer.
//
// Returns true if the value is an actual integer (not a float).
// This test is also registered under the alias "int".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("integer", TestInteger)
//
// Template usage:
//
//	{{ 42 is integer }}
//	  -> true
//	{{ 42.0 is integer }}
//	  -> false
func TestInteger(_ *State, val value.Value, _ []value.Value) (bool, error) {
	_, ok := val.AsInt()
	if !ok {
		return false, nil
	}
	return val.IsActualInt(), nil
}

// TestFloat checks if a value is a float.
//
// Returns true if the value is a floating-point number.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("float", TestFloat)
//
// Template usage:
//
//	{{ 42.0 is float }}
//	  -> true
//	{{ 42 is float }}
//	  -> false
func TestFloat(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsActualFloat(), nil
}

// TestBoolean checks if a value is a boolean.
//
// Returns true if the value is a boolean (true or false).
// This test is also registered under the alias "bool".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("boolean", TestBoolean)
//
// Template usage:
//
//	{{ true is boolean }}
//	  -> true
//	{{ 1 is boolean }}
//	  -> false
func TestBoolean(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindBool, nil
}

// TestSafe checks if a value is marked as safe.
//
// Returns true if the value has been marked as safe for auto-escaping.
// This test is also registered under the alias "escaped".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("safe", TestSafe)
//
// Template usage:
//
//	{{ "<hello>"|escape is safe }}
//	  -> true
func TestSafe(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsSafe(), nil
}

// TestSameAs checks if two values are the exact same object.
//
// This is a stricter comparison than equality. Values that have the same
// structure but are different objects will not compare as "same".
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("sameas", TestSameAs)
//
// Template usage:
//
//	{{ [1, 2, 3] is sameas([1, 2, 3]) }}
//	  -> false
//	{{ false is sameas(false) }}
//	  -> true
func TestSameAs(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.SameAs(args[0]), nil
}

// TestLower checks if a string is all lowercase.
//
// Returns true if all alphabetic characters in the string are lowercase.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("lower", TestLower)
//
// Template usage:
//
//	{{ "foo" is lower }}
//	  -> true
//	{{ "Foo" is lower }}
//	  -> false
func TestLower(_ *State, val value.Value, _ []value.Value) (bool, error) {
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

// TestUpper checks if a string is all uppercase.
//
// Returns true if all alphabetic characters in the string are uppercase.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("upper", TestUpper)
//
// Template usage:
//
//	{{ "FOO" is upper }}
//	  -> true
//	{{ "Foo" is upper }}
//	  -> false
func TestUpper(_ *State, val value.Value, _ []value.Value) (bool, error) {
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

// TestFilter checks if a filter with the given name exists.
//
// This is useful for checking whether certain template features are available.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("filter", TestFilter)
//
// Template usage:
//
//	{% if "tojson" is filter %}
//	  JSON serialization available
//	{% endif %}
func TestFilter(state *State, val value.Value, _ []value.Value) (bool, error) {
	name, ok := val.AsString()
	if !ok {
		return false, nil
	}
	_, exists := state.env.getFilter(name)
	return exists, nil
}

// TestTest checks if a test with the given name exists.
//
// This is useful for checking whether certain template features are available.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("test", TestTest)
//
// Template usage:
//
//	{% if "greaterthan" is test %}
//	  Comparison tests available
//	{% endif %}
func TestTest(state *State, val value.Value, _ []value.Value) (bool, error) {
	name, ok := val.AsString()
	if !ok {
		return false, nil
	}
	_, exists := state.env.getTest(name)
	return exists, nil
}

// TestSequence checks if a value is a sequence.
//
// Returns true if the value is a list/array.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("sequence", TestSequence)
//
// Template usage:
//
//	{{ [1, 2, 3] is sequence }}
//	  -> true
//	{{ 42 is sequence }}
//	  -> false
func TestSequence(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindSeq, nil
}

// TestMapping checks if a value is a mapping/dict.
//
// Returns true if the value is a map/dictionary.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("mapping", TestMapping)
//
// Template usage:
//
//	{{ {"foo": "bar"} is mapping }}
//	  -> true
//	{{ [1, 2, 3] is mapping }}
//	  -> false
func TestMapping(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindMap, nil
}

// TestIterable checks if a value can be iterated over.
//
// Returns true if the value supports iteration (sequences, maps, strings, etc.).
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("iterable", TestIterable)
//
// Template usage:
//
//	{{ [1, 2, 3] is iterable }}
//	  -> true
//	{{ 42 is iterable }}
//	  -> false
func TestIterable(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Iter() != nil, nil
}

// TestStartingWith checks if a string starts with a given prefix.
//
// Returns true if the string starts with the specified prefix.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("startingwith", TestStartingWith)
//
// Template usage:
//
//	{{ "foobar" is startingwith("foo") }}
//	  -> true
//	{{ "foobar" is startingwith("bar") }}
//	  -> false
func TestStartingWith(_ *State, val value.Value, args []value.Value) (bool, error) {
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

// TestEndingWith checks if a string ends with a given suffix.
//
// Returns true if the string ends with the specified suffix.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("endingwith", TestEndingWith)
//
// Template usage:
//
//	{{ "foobar" is endingwith("bar") }}
//	  -> true
//	{{ "foobar" is endingwith("foo") }}
//	  -> false
func TestEndingWith(_ *State, val value.Value, args []value.Value) (bool, error) {
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

// TestContaining checks if a value contains another value.
//
// For strings, this checks if the substring is present. For sequences and
// maps, it checks if the item or key is present.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("containing", TestContaining)
//
// Template usage:
//
//	{{ "foobar" is containing("oob") }}
//	  -> true
//	{{ [1, 2, 3] is containing(2) }}
//	  -> true
func TestContaining(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.Contains(args[0]), nil
}
