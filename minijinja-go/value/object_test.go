package value

import (
	"context"
	"iter"
	"slices"
	"sync/atomic"
	"testing"
)

// mockState implements State for testing
type mockState struct{}

func (m *mockState) Context() context.Context { return context.Background() }
func (m *mockState) Lookup(name string) Value { return Undefined() }
func (m *mockState) Name() string             { return "test" }

// -----------------------------------------------------------------------------
// Test ObjectRepr
// -----------------------------------------------------------------------------

func TestObjectRepr(t *testing.T) {
	tests := []struct {
		repr ObjectRepr
		want string
	}{
		{ObjectReprPlain, "plain"},
		{ObjectReprMap, "map"},
		{ObjectReprSeq, "seq"},
		{ObjectReprIterable, "iterable"},
		{ObjectRepr(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.repr.String(); got != tt.want {
			t.Errorf("ObjectRepr(%d).String() = %q, want %q", tt.repr, got, tt.want)
		}
	}
}

// -----------------------------------------------------------------------------
// Test SeqObject
// -----------------------------------------------------------------------------

type testSeq struct {
	items []Value
}

func (s *testSeq) GetAttr(name string) Value { return Undefined() }
func (s *testSeq) ObjectRepr() ObjectRepr    { return ObjectReprSeq }
func (s *testSeq) SeqLen() int               { return len(s.items) }
func (s *testSeq) SeqItem(i int) Value {
	if i >= 0 && i < len(s.items) {
		return s.items[i]
	}
	return Undefined()
}

func TestSeqObject(t *testing.T) {
	seq := &testSeq{items: []Value{FromInt(1), FromInt(2), FromInt(3)}}
	val := FromObject(seq)

	// Test length
	if l, ok := val.Len(); !ok || l != 3 {
		t.Errorf("Len() = %d, %v, want 3, true", l, ok)
	}

	// Test GetItem
	if item := val.GetItem(FromInt(0)); item.String() != "1" {
		t.Errorf("GetItem(0) = %v, want 1", item)
	}
	if item := val.GetItem(FromInt(2)); item.String() != "3" {
		t.Errorf("GetItem(2) = %v, want 3", item)
	}
	if item := val.GetItem(FromInt(10)); !item.IsUndefined() {
		t.Errorf("GetItem(10) = %v, want undefined", item)
	}

	// Test iteration
	var got []int64
	for v := range IterateObject(seq) {
		i, _ := v.AsInt()
		got = append(got, i)
	}
	if !slices.Equal(got, []int64{1, 2, 3}) {
		t.Errorf("Iteration = %v, want [1 2 3]", got)
	}
}

// -----------------------------------------------------------------------------
// Test MapObject
// -----------------------------------------------------------------------------

type testMap struct {
	data map[string]Value
	keys []string
}

func (m *testMap) GetAttr(name string) Value {
	if v, ok := m.data[name]; ok {
		return v
	}
	return Undefined()
}
func (m *testMap) Keys() []string { return m.keys }

func TestMapObject(t *testing.T) {
	tm := &testMap{
		data: map[string]Value{"a": FromInt(1), "b": FromInt(2)},
		keys: []string{"a", "b"},
	}
	val := FromObject(tm)

	// Test length
	if l, ok := val.Len(); !ok || l != 2 {
		t.Errorf("Len() = %d, %v, want 2, true", l, ok)
	}

	// Test GetAttr
	if v := val.GetAttr("a"); v.String() != "1" {
		t.Errorf("GetAttr(a) = %v, want 1", v)
	}
	if v := val.GetAttr("missing"); !v.IsUndefined() {
		t.Errorf("GetAttr(missing) = %v, want undefined", v)
	}

	// Test iteration (should yield keys)
	var got []string
	for v := range IterateObject(tm) {
		s, _ := v.AsString()
		got = append(got, s)
	}
	if !slices.Equal(got, []string{"a", "b"}) {
		t.Errorf("Iteration = %v, want [a b]", got)
	}
}

// -----------------------------------------------------------------------------
// Test CallableObject
// -----------------------------------------------------------------------------

type testCallable struct {
	callCount atomic.Int64
}

func (c *testCallable) GetAttr(name string) Value { return Undefined() }
func (c *testCallable) ObjectCall(state State, args []Value, kwargs map[string]Value) (Value, error) {
	count := c.callCount.Add(1)
	return FromInt(count), nil
}

func TestCallableObject(t *testing.T) {
	c := &testCallable{}
	val := FromObject(c)

	// Check it's recognized as callable
	if callable, ok := val.AsCallable(); !ok {
		t.Error("AsCallable() returned false")
	} else {
		// Call it multiple times
		state := &mockState{}
		r1, _ := callable.Call(state, nil, nil)
		r2, _ := callable.Call(state, nil, nil)
		if i1, _ := r1.AsInt(); i1 != 1 {
			t.Errorf("First call = %d, want 1", i1)
		}
		if i2, _ := r2.AsInt(); i2 != 2 {
			t.Errorf("Second call = %d, want 2", i2)
		}
	}
}

// -----------------------------------------------------------------------------
// Test MethodCallable
// -----------------------------------------------------------------------------

type testMethodCallable struct{}

func (m *testMethodCallable) GetAttr(name string) Value { return Undefined() }
func (m *testMethodCallable) CallMethod(state State, name string, args []Value, kwargs map[string]Value) (Value, error) {
	if name == "greet" {
		if len(args) > 0 {
			s, _ := args[0].AsString()
			return FromString("Hello, " + s + "!"), nil
		}
		return FromString("Hello!"), nil
	}
	return Undefined(), ErrUnknownMethod
}

func TestMethodCallable(t *testing.T) {
	m := &testMethodCallable{}

	// Test known method
	result, err := m.CallMethod(&mockState{}, "greet", []Value{FromString("World")}, nil)
	if err != nil {
		t.Errorf("CallMethod(greet) error: %v", err)
	}
	if s, _ := result.AsString(); s != "Hello, World!" {
		t.Errorf("CallMethod(greet) = %q, want %q", s, "Hello, World!")
	}

	// Test unknown method
	_, err = m.CallMethod(&mockState{}, "unknown", nil, nil)
	if err != ErrUnknownMethod {
		t.Errorf("CallMethod(unknown) error = %v, want ErrUnknownMethod", err)
	}
}

// -----------------------------------------------------------------------------
// Test ObjectWithCmp
// -----------------------------------------------------------------------------

type testComparable struct {
	num int
}

func (c *testComparable) GetAttr(name string) Value { return Undefined() }
func (c *testComparable) ObjectCmp(other Object) (int, bool) {
	if oc, ok := other.(*testComparable); ok {
		return c.num - oc.num, true
	}
	return 0, false
}

func TestObjectWithCmp(t *testing.T) {
	a := &testComparable{num: 10}
	b := &testComparable{num: 20}
	c := &testComparable{num: 10}

	// Test CompareObjects helper
	if cmp, ok := CompareObjects(a, b); !ok || cmp >= 0 {
		t.Errorf("CompareObjects(10, 20) = %d, %v, want <0, true", cmp, ok)
	}
	if cmp, ok := CompareObjects(b, a); !ok || cmp <= 0 {
		t.Errorf("CompareObjects(20, 10) = %d, %v, want >0, true", cmp, ok)
	}
	if cmp, ok := CompareObjects(a, c); !ok || cmp != 0 {
		t.Errorf("CompareObjects(10, 10) = %d, %v, want 0, true", cmp, ok)
	}

	// Test via Value.Compare
	va, vb := FromObject(a), FromObject(b)
	if cmp, ok := va.Compare(vb); !ok || cmp >= 0 {
		t.Errorf("Value.Compare(10, 20) = %d, %v, want <0, true", cmp, ok)
	}

	// Test incomparable types
	incomp := &testSeq{items: nil}
	if _, ok := CompareObjects(a, incomp); ok {
		t.Error("CompareObjects(comparable, seq) should return ok=false")
	}
}

// -----------------------------------------------------------------------------
// Test ReversibleObject
// -----------------------------------------------------------------------------

type testReversible struct {
	items []int
}

func (r *testReversible) GetAttr(name string) Value { return Undefined() }
func (r *testReversible) ObjectRepr() ObjectRepr    { return ObjectReprSeq }
func (r *testReversible) SeqLen() int               { return len(r.items) }
func (r *testReversible) SeqItem(i int) Value {
	if i >= 0 && i < len(r.items) {
		return FromInt(int64(r.items[i]))
	}
	return Undefined()
}
func (r *testReversible) ReverseIterate() iter.Seq[Value] {
	return func(yield func(Value) bool) {
		for i := len(r.items) - 1; i >= 0; i-- {
			if !yield(FromInt(int64(r.items[i]))) {
				return
			}
		}
	}
}

func TestReversibleObject(t *testing.T) {
	r := &testReversible{items: []int{1, 2, 3, 4, 5}}

	// Forward iteration
	var forward []int64
	for v := range IterateObject(r) {
		i, _ := v.AsInt()
		forward = append(forward, i)
	}
	if !slices.Equal(forward, []int64{1, 2, 3, 4, 5}) {
		t.Errorf("Forward iteration = %v, want [1 2 3 4 5]", forward)
	}

	// Reverse iteration
	var reverse []int64
	for v := range ReverseIterateObject(r) {
		i, _ := v.AsInt()
		reverse = append(reverse, i)
	}
	if !slices.Equal(reverse, []int64{5, 4, 3, 2, 1}) {
		t.Errorf("Reverse iteration = %v, want [5 4 3 2 1]", reverse)
	}
}

func TestReverseIterateSeqObject(t *testing.T) {
	// Test that SeqObject can be reversed without ReversibleObject
	seq := &testSeq{items: []Value{FromInt(1), FromInt(2), FromInt(3)}}

	var reverse []int64
	for v := range ReverseIterateObject(seq) {
		i, _ := v.AsInt()
		reverse = append(reverse, i)
	}
	if !slices.Equal(reverse, []int64{3, 2, 1}) {
		t.Errorf("Reverse iteration = %v, want [3 2 1]", reverse)
	}
}

func TestReverseIterateMapObject(t *testing.T) {
	tm := &testMap{
		data: map[string]Value{"a": FromInt(1), "b": FromInt(2), "c": FromInt(3)},
		keys: []string{"a", "b", "c"},
	}

	var reverse []string
	for v := range ReverseIterateObject(tm) {
		s, _ := v.AsString()
		reverse = append(reverse, s)
	}
	if !slices.Equal(reverse, []string{"c", "b", "a"}) {
		t.Errorf("Reverse iteration = %v, want [c b a]", reverse)
	}
}

// -----------------------------------------------------------------------------
// Test MakeIterable
// -----------------------------------------------------------------------------

func TestMakeIterable(t *testing.T) {
	callCount := 0
	val := MakeIterable(func() iter.Seq[Value] {
		callCount++
		return func(yield func(Value) bool) {
			for i := 0; i < 5; i++ {
				if !yield(FromInt(int64(i))) {
					return
				}
			}
		}
	})

	// First iteration
	var first []int64
	for v := range IterateObject(val.data.(Object)) {
		i, _ := v.AsInt()
		first = append(first, i)
	}
	if !slices.Equal(first, []int64{0, 1, 2, 3, 4}) {
		t.Errorf("First iteration = %v, want [0 1 2 3 4]", first)
	}

	// Second iteration (should work again)
	var second []int64
	for v := range IterateObject(val.data.(Object)) {
		i, _ := v.AsInt()
		second = append(second, i)
	}
	if !slices.Equal(second, []int64{0, 1, 2, 3, 4}) {
		t.Errorf("Second iteration = %v, want [0 1 2 3 4]", second)
	}

	// Maker should have been called twice
	if callCount != 2 {
		t.Errorf("Maker call count = %d, want 2", callCount)
	}
}

func TestMakeIterableFromSlice(t *testing.T) {
	val := MakeIterableFromSlice(func() []Value {
		return []Value{FromString("a"), FromString("b"), FromString("c")}
	})

	var got []string
	for v := range IterateObject(val.data.(Object)) {
		s, _ := v.AsString()
		got = append(got, s)
	}
	if !slices.Equal(got, []string{"a", "b", "c"}) {
		t.Errorf("Iteration = %v, want [a b c]", got)
	}
}

// -----------------------------------------------------------------------------
// Test MakeOneShotIterator
// -----------------------------------------------------------------------------

func TestOneShotIterator_FullConsumption(t *testing.T) {
	val := MakeOneShotIterator(func(yield func(Value) bool) {
		for i := 0; i < 5; i++ {
			if !yield(FromInt(int64(i))) {
				return
			}
		}
	})

	obj := val.data.(Object)

	// Verify no known length
	if l := GetObjectLen(obj); l != -1 {
		t.Errorf("Length = %d, want -1 (unknown)", l)
	}

	// First iteration - should yield all items
	var first []int64
	for v := range IterateObject(obj) {
		i, _ := v.AsInt()
		first = append(first, i)
	}
	if !slices.Equal(first, []int64{0, 1, 2, 3, 4}) {
		t.Errorf("First iteration = %v, want [0 1 2 3 4]", first)
	}

	// Second iteration - should yield nothing
	var second []int64
	for v := range IterateObject(obj) {
		i, _ := v.AsInt()
		second = append(second, i)
	}
	if len(second) != 0 {
		t.Errorf("Second iteration = %v, want []", second)
	}
}

func TestOneShotIterator_PartialConsumption(t *testing.T) {
	val := MakeOneShotIterator(func(yield func(Value) bool) {
		for i := 0; i < 5; i++ {
			if !yield(FromInt(int64(i))) {
				return
			}
		}
	})

	obj := val.data.(Object)

	// First iteration - stop after 2 items
	var first []int64
	count := 0
	for v := range IterateObject(obj) {
		i, _ := v.AsInt()
		first = append(first, i)
		count++
		if count == 2 {
			break
		}
	}
	if !slices.Equal(first, []int64{0, 1}) {
		t.Errorf("First iteration = %v, want [0 1]", first)
	}

	// Second iteration - should yield remaining items
	var second []int64
	for v := range IterateObject(obj) {
		i, _ := v.AsInt()
		second = append(second, i)
	}
	if !slices.Equal(second, []int64{2, 3, 4}) {
		t.Errorf("Second iteration = %v, want [2 3 4]", second)
	}

	// Third iteration - should yield nothing
	var third []int64
	for v := range IterateObject(obj) {
		i, _ := v.AsInt()
		third = append(third, i)
	}
	if len(third) != 0 {
		t.Errorf("Third iteration = %v, want []", third)
	}
}

func TestOneShotIterator_String(t *testing.T) {
	val := MakeOneShotIterator(func(yield func(Value) bool) {
		yield(FromInt(1))
	})

	if s := val.String(); s != "<iterator>" {
		t.Errorf("String() = %q, want %q", s, "<iterator>")
	}
}

// -----------------------------------------------------------------------------
// Test MakeObjectMap
// -----------------------------------------------------------------------------

func TestMakeObjectMap(t *testing.T) {
	data := map[string]int{"a": 1, "b": 2, "c": 3}
	keys := []string{"a", "b", "c"}

	val := MakeObjectMap(
		func() iter.Seq[Value] {
			return func(yield func(Value) bool) {
				for _, k := range keys {
					if !yield(FromString(k)) {
						return
					}
				}
			}
		},
		func(key Value) Value {
			if s, ok := key.AsString(); ok {
				if v, exists := data[s]; exists {
					return FromInt(int64(v))
				}
			}
			return Undefined()
		},
	)

	obj := val.data.(Object)

	// Test length
	if l := GetObjectLen(obj); l != 3 {
		t.Errorf("Length = %d, want 3", l)
	}

	// Test GetAttr
	if v := val.GetAttr("a"); v.String() != "1" {
		t.Errorf("GetAttr(a) = %v, want 1", v)
	}
	if v := val.GetAttr("missing"); !v.IsUndefined() {
		t.Errorf("GetAttr(missing) = %v, want undefined", v)
	}

	// Test iteration (yields keys)
	var got []string
	for v := range IterateObject(obj) {
		s, _ := v.AsString()
		got = append(got, s)
	}
	if !slices.Equal(got, keys) {
		t.Errorf("Iteration = %v, want %v", got, keys)
	}

	// Test GetItem (via objectMapProjection.GetItem)
	if proj, ok := obj.(*objectMapProjection); ok {
		if v := proj.GetItem(FromString("b")); v.String() != "2" {
			t.Errorf("GetItem(b) = %v, want 2", v)
		}
	}
}

// -----------------------------------------------------------------------------
// Test ObjectWithLen
// -----------------------------------------------------------------------------

type testWithLen struct {
	length int
}

func (l *testWithLen) GetAttr(name string) Value { return Undefined() }
func (l *testWithLen) ObjectLen() int            { return l.length }

func TestObjectWithLen(t *testing.T) {
	obj := &testWithLen{length: 42}
	if l := GetObjectLen(obj); l != 42 {
		t.Errorf("GetObjectLen() = %d, want 42", l)
	}

	// Unknown length
	obj2 := &testWithLen{length: -1}
	if l := GetObjectLen(obj2); l != -1 {
		t.Errorf("GetObjectLen() = %d, want -1", l)
	}
}

// -----------------------------------------------------------------------------
// Test ObjectWithTruth
// -----------------------------------------------------------------------------

type testWithTruth struct {
	truth bool
}

func (t *testWithTruth) GetAttr(name string) Value { return Undefined() }
func (t *testWithTruth) ObjectIsTrue() bool        { return t.truth }

func TestObjectWithTruth(t *testing.T) {
	truthy := &testWithTruth{truth: true}
	falsy := &testWithTruth{truth: false}

	if !GetObjectTruth(truthy) {
		t.Error("GetObjectTruth(true) = false, want true")
	}
	if GetObjectTruth(falsy) {
		t.Error("GetObjectTruth(false) = true, want false")
	}
}

func TestObjectTruthFromLength(t *testing.T) {
	// Empty sequence is falsy
	emptySeq := &testSeq{items: nil}
	if GetObjectTruth(emptySeq) {
		t.Error("GetObjectTruth(empty seq) = true, want false")
	}

	// Non-empty sequence is truthy
	nonEmpty := &testSeq{items: []Value{FromInt(1)}}
	if !GetObjectTruth(nonEmpty) {
		t.Error("GetObjectTruth(non-empty seq) = false, want true")
	}
}

// -----------------------------------------------------------------------------
// Test ObjectWithString
// -----------------------------------------------------------------------------

type testWithString struct {
	str string
}

func (s *testWithString) GetAttr(name string) Value { return Undefined() }
func (s *testWithString) ObjectString() string      { return s.str }

func TestObjectWithString(t *testing.T) {
	obj := &testWithString{str: "custom string"}
	val := FromObject(obj)

	// ObjectWithString is checked in Value.String()
	var objInterface Object = obj
	if stringer, ok := objInterface.(ObjectWithString); ok {
		if s := stringer.ObjectString(); s != "custom string" {
			t.Errorf("ObjectString() = %q, want %q", s, "custom string")
		}
	} else {
		t.Error("obj should implement ObjectWithString")
	}

	// Note: Value.String() would need to check for ObjectWithString
	// to use the custom string representation
	_ = val
}

// -----------------------------------------------------------------------------
// Test GetObjectRepr default
// -----------------------------------------------------------------------------

type plainObject struct{}

func (p *plainObject) GetAttr(name string) Value { return Undefined() }

func TestGetObjectReprDefault(t *testing.T) {
	plain := &plainObject{}
	if r := GetObjectRepr(plain); r != ObjectReprPlain {
		t.Errorf("GetObjectRepr(plain) = %v, want ObjectReprPlain", r)
	}
}

// -----------------------------------------------------------------------------
// Test IterateObject returns nil for non-iterable
// -----------------------------------------------------------------------------

func TestIterateObjectNonIterable(t *testing.T) {
	plain := &plainObject{}
	if iter := IterateObject(plain); iter != nil {
		t.Error("IterateObject(plain) should return nil")
	}
}

// -----------------------------------------------------------------------------
// Test ReverseIterateObject returns nil for non-reversible plain object
// -----------------------------------------------------------------------------

func TestReverseIterateObjectNonReversible(t *testing.T) {
	plain := &plainObject{}
	if iter := ReverseIterateObject(plain); iter != nil {
		t.Error("ReverseIterateObject(plain) should return nil")
	}
}
