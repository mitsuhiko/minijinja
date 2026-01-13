package minijinja

import (
	"fmt"
	"strings"
	"unicode"

	"github.com/mitsuhiko/minijinja/minijinja-go/value"
)

// Built-in test implementations. These mirror the Rust MiniJinja tests.

// testDefined implements the built-in `defined` test.
func testDefined(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return !val.IsUndefined(), nil
}

// testUndefined implements the built-in `undefined` test.
func testUndefined(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsUndefined(), nil
}

// testNone implements the built-in `none` test.
func testNone(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsNone(), nil
}

// testTrue implements the built-in `true` test.
func testTrue(_ *State, val value.Value, _ []value.Value) (bool, error) {
	if b, ok := val.AsBool(); ok {
		return b, nil
	}
	return false, nil
}

// testFalse implements the built-in `false` test.
func testFalse(_ *State, val value.Value, _ []value.Value) (bool, error) {
	if b, ok := val.AsBool(); ok {
		return !b, nil
	}
	return false, nil
}

// testOdd implements the built-in `odd` test.
func testOdd(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) > 0 {
		return false, NewError(ErrInvalidOperation, "odd test expects no arguments")
	}
	if i, ok := val.AsInt(); ok {
		return i%2 != 0, nil
	}
	return false, nil
}

// testEven implements the built-in `even` test.
func testEven(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) > 0 {
		return false, NewError(ErrInvalidOperation, "even test expects no arguments")
	}
	if i, ok := val.AsInt(); ok {
		return i%2 == 0, nil
	}
	return false, nil
}

// testDivisibleBy implements the built-in `divisibleby` test.
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

// testEq implements the built-in `eq`/`equalto` test.
func testEq(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.Equal(args[0]), nil
}

// testNe implements the built-in `ne` test.
func testNe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return !val.Equal(args[0]), nil
}

// testLt implements the built-in `lt` test.
func testLt(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp < 0, nil
	}
	return false, nil
}

// testLe implements the built-in `le` test.
func testLe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp <= 0, nil
	}
	return false, nil
}

// testGt implements the built-in `gt` test.
func testGt(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp > 0, nil
	}
	return false, nil
}

// testGe implements the built-in `ge` test.
func testGe(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	if cmp, ok := val.Compare(args[0]); ok {
		return cmp >= 0, nil
	}
	return false, nil
}

// testIn implements the built-in `in` test.
func testIn(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return args[0].Contains(val), nil
}

// testString implements the built-in `string` test.
func testString(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindString, nil
}

// testNumber implements the built-in `number` test.
func testNumber(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindNumber, nil
}

// testInteger implements the built-in `integer`/`int` test.
func testInteger(_ *State, val value.Value, _ []value.Value) (bool, error) {
	_, ok := val.AsInt()
	if !ok {
		return false, nil
	}
	return val.IsActualInt(), nil
}

// testFloat implements the built-in `float` test.
func testFloat(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsActualFloat(), nil
}

// testBoolean implements the built-in `boolean` test.
func testBoolean(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindBool, nil
}

// testSafe implements the built-in `safe`/`escaped` test.
func testSafe(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.IsSafe(), nil
}

// testSameAs implements the built-in `sameas` test.
func testSameAs(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.SameAs(args[0]), nil
}

// testLower implements the built-in `lower` test.
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

// testUpper implements the built-in `upper` test.
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

// testFilter implements the built-in `filter` test.
func testFilter(state *State, val value.Value, _ []value.Value) (bool, error) {
	name, ok := val.AsString()
	if !ok {
		return false, nil
	}
	_, exists := state.env.getFilter(name)
	return exists, nil
}

// testTest implements the built-in `test` test.
func testTest(state *State, val value.Value, _ []value.Value) (bool, error) {
	name, ok := val.AsString()
	if !ok {
		return false, nil
	}
	_, exists := state.env.getTest(name)
	return exists, nil
}

// testSequence implements the built-in `sequence` test.
func testSequence(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindSeq, nil
}

// testMapping implements the built-in `mapping` test.
func testMapping(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Kind() == value.KindMap, nil
}

// testIterable implements the built-in `iterable` test.
func testIterable(_ *State, val value.Value, _ []value.Value) (bool, error) {
	return val.Iter() != nil, nil
}

// testStartingWith implements the built-in `startingwith` test.
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

// testEndingWith implements the built-in `endingwith` test.
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

// testContaining implements the built-in `containing` test.
func testContaining(_ *State, val value.Value, args []value.Value) (bool, error) {
	if len(args) < 1 {
		return false, nil
	}
	return val.Contains(args[0]), nil
}
