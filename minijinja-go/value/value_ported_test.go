// Tests ported from Rust's minijinja/tests/test_value.rs
package value

import (
	"iter"
	"slices"
	"sort"
	"testing"
)

// -----------------------------------------------------------------------------
// Basic Value Tests (ported from test_value.rs)
// -----------------------------------------------------------------------------

func TestValueByIndex(t *testing.T) {
	val := FromSlice([]Value{FromInt(1), FromInt(2), FromInt(3)})

	item0 := val.GetItem(FromInt(0))
	if i, ok := item0.AsInt(); !ok || i != 1 {
		t.Errorf("GetItem(0) = %v, want 1", item0)
	}

	item4 := val.GetItem(FromInt(4))
	if !item4.IsUndefined() {
		t.Errorf("GetItem(4) = %v, want undefined", item4)
	}
}

func TestSafeString(t *testing.T) {
	v := FromSafeString("<b>test</b>")
	v2 := FromSafeString("<b>test</b>")

	if !v.IsSafe() {
		t.Error("v should be safe")
	}
	if !v2.IsSafe() {
		t.Error("v2 should be safe")
	}
	if v.String() != v2.String() {
		t.Errorf("v.String() = %q, v2.String() = %q, should be equal", v.String(), v2.String())
	}
}

func TestUndefined(t *testing.T) {
	v := Undefined()
	v2 := Undefined()

	if !v.IsUndefined() {
		t.Error("v should be undefined")
	}
	if !v2.IsUndefined() {
		t.Error("v2 should be undefined")
	}
}

func TestFloatToString(t *testing.T) {
	tests := []struct {
		val  Value
		want string
	}{
		{FromFloat(42.4242), "42.4242"},
		{FromFloat(42.0), "42.0"},
	}
	for _, tt := range tests {
		if got := tt.val.String(); got != tt.want {
			t.Errorf("FromFloat(%v).String() = %q, want %q", tt.val, got, tt.want)
		}
	}
}

// -----------------------------------------------------------------------------
// Equality Tests (ported from test_value.rs)
// -----------------------------------------------------------------------------

func TestValueEquality(t *testing.T) {
	// Same slices are equal
	s1 := FromSlice([]Value{FromInt(1)})
	s2 := FromSlice([]Value{FromInt(1)})
	if !s1.Equal(s2) {
		t.Error("slices [1] should be equal")
	}

	// Different slices are not equal
	s3 := FromSlice([]Value{FromInt(2)})
	if s1.Equal(s3) {
		t.Error("slices [1] and [2] should not be equal")
	}

	// Undefined equals undefined
	if !Undefined().Equal(Undefined()) {
		t.Error("undefined should equal undefined")
	}
}

func TestFloatEquality(t *testing.T) {
	// Large int equals equivalent float
	a := FromInt(1 << 53)
	b := FromFloat(float64(1 << 53))
	if !a.Equal(b) {
		t.Errorf("%v should equal %v", a, b)
	}
}

// -----------------------------------------------------------------------------
// Comparison Tests (ported from test_value.rs)
// -----------------------------------------------------------------------------

func TestValueComparison(t *testing.T) {
	tests := []struct {
		a, b Value
		want int // -1, 0, 1
	}{
		{FromInt(1), FromInt(2), -1},
		{FromInt(2), FromInt(1), 1},
		{FromInt(1), FromInt(1), 0},
		{FromString("a"), FromString("b"), -1},
		{FromString("b"), FromString("a"), 1},
		{FromBool(false), FromBool(true), -1},
		{FromBool(true), FromBool(false), 1},
	}

	for _, tt := range tests {
		cmp, ok := tt.a.Compare(tt.b)
		if !ok {
			t.Errorf("Compare(%v, %v) failed", tt.a, tt.b)
			continue
		}
		if cmp != tt.want {
			t.Errorf("Compare(%v, %v) = %d, want %d", tt.a, tt.b, cmp, tt.want)
		}
	}
}

func TestSequenceComparison(t *testing.T) {
	v1 := FromSlice([]Value{FromInt(1), FromInt(2), FromInt(3), FromInt(4)})
	v2 := FromSlice([]Value{FromInt(1), FromInt(2), FromInt(3)})

	cmp, ok := v1.Compare(v2)
	if !ok {
		t.Error("Compare should succeed for sequences")
	}
	if cmp <= 0 {
		t.Errorf("longer sequence should be greater: got %d", cmp)
	}
}

// -----------------------------------------------------------------------------
// Sorting Tests (ported from test_value.rs test_sort and test_sorting)
// -----------------------------------------------------------------------------

func TestValueSort(t *testing.T) {
	values := []Value{
		FromInt(100),
		FromInt(80),
		FromInt(30),
		FromBool(true),
		FromBool(false),
		FromInt(99),
		FromFloat(1000.0),
	}

	sort.Slice(values, func(i, j int) bool {
		cmp, _ := values[i].Compare(values[j])
		return cmp < 0
	})

	// Expected order: false, true, 30, 80, 99, 100, 1000.0
	expected := []string{"false", "true", "30", "80", "99", "100", "1000.0"}
	for i, v := range values {
		if v.String() != expected[i] {
			t.Errorf("values[%d] = %v, want %s", i, v.String(), expected[i])
		}
	}
}

func TestSortDifferentTypes(t *testing.T) {
	// Kind ordering: Undefined < None < Bool < Number < String < Seq < Map
	values := []Value{
		FromInt(100),
		FromString("bar"),
		FromBool(true),
		Undefined(),
		FromString("foo"),
		None(),
		FromInt(0),
	}

	sort.Slice(values, func(i, j int) bool {
		cmp, _ := values[i].Compare(values[j])
		return cmp < 0
	})

	// Check kind ordering
	kinds := make([]ValueKind, len(values))
	for i, v := range values {
		kinds[i] = v.Kind()
	}

	expectedKinds := []ValueKind{
		KindUndefined, KindNone, KindBool, KindNumber, KindNumber, KindString, KindString,
	}
	for i, k := range kinds {
		if k != expectedKinds[i] {
			t.Errorf("kinds[%d] = %v, want %v", i, k, expectedKinds[i])
		}
	}
}

// -----------------------------------------------------------------------------
// Object Tests (ported from test_value.rs)
// -----------------------------------------------------------------------------

// MapPoint is a map-like object
type MapPoint struct {
	x, y, z int
}

func (p *MapPoint) GetAttr(name string) Value {
	switch name {
	case "x":
		return FromInt(int64(p.x))
	case "y":
		return FromInt(int64(p.y))
	case "z":
		return FromInt(int64(p.z))
	}
	return Undefined()
}

func (p *MapPoint) Keys() []string {
	return []string{"x", "y", "z"}
}

func TestMapObjectIteration(t *testing.T) {
	point := &MapPoint{x: 1, y: 2, z: 3}
	val := FromObject(point)

	// Check iteration yields keys
	var keys []string
	for v := range IterateObject(point) {
		s, _ := v.AsString()
		keys = append(keys, s)
	}

	if !slices.Equal(keys, []string{"x", "y", "z"}) {
		t.Errorf("iteration keys = %v, want [x y z]", keys)
	}

	// Check attribute access
	if x := val.GetAttr("x"); x.String() != "1" {
		t.Errorf("GetAttr(x) = %v, want 1", x)
	}
	if missing := val.GetAttr("missing"); !missing.IsUndefined() {
		t.Errorf("GetAttr(missing) = %v, want undefined", missing)
	}
}

// SeqPoint is a sequence-like object
type SeqPoint struct {
	coords [3]int
}

func (p *SeqPoint) GetAttr(name string) Value {
	return Undefined()
}

func (p *SeqPoint) ObjectRepr() ObjectRepr {
	return ObjectReprSeq
}

func (p *SeqPoint) SeqLen() int {
	return 3
}

func (p *SeqPoint) SeqItem(index int) Value {
	if index >= 0 && index < 3 {
		return FromInt(int64(p.coords[index]))
	}
	return Undefined()
}

func TestSeqObjectIteration(t *testing.T) {
	point := &SeqPoint{coords: [3]int{1, 2, 3}}
	val := FromObject(point)

	// Check iteration yields values
	var values []int64
	for v := range IterateObject(point) {
		i, _ := v.AsInt()
		values = append(values, i)
	}

	if !slices.Equal(values, []int64{1, 2, 3}) {
		t.Errorf("iteration values = %v, want [1 2 3]", values)
	}

	// Check index access
	if v := val.GetItem(FromInt(0)); v.String() != "1" {
		t.Errorf("GetItem(0) = %v, want 1", v)
	}
	if v := val.GetItem(FromInt(42)); !v.IsUndefined() {
		t.Errorf("GetItem(42) = %v, want undefined", v)
	}
}

// -----------------------------------------------------------------------------
// Custom Object Comparison (ported from test_custom_object_compare)
// -----------------------------------------------------------------------------

type ComparableNum struct {
	n int
}

func (c *ComparableNum) GetAttr(name string) Value {
	return Undefined()
}

func (c *ComparableNum) ObjectCmp(other Object) (int, bool) {
	if o, ok := other.(*ComparableNum); ok {
		return c.n - o.n, true
	}
	return 0, false
}

func (c *ComparableNum) ObjectString() string {
	return FromInt(int64(c.n)).String()
}

func TestCustomObjectCompare(t *testing.T) {
	// Create values 4, 3, 2, 1, 0
	nums := make([]Value, 5)
	for i := 0; i < 5; i++ {
		nums[i] = FromObject(&ComparableNum{n: 4 - i})
	}

	// Sort them
	sort.Slice(nums, func(i, j int) bool {
		cmp, ok := nums[i].Compare(nums[j])
		if !ok {
			return false
		}
		return cmp < 0
	})

	// Should be 0, 1, 2, 3, 4
	var result []int
	for _, v := range nums {
		if obj, ok := v.AsObject(); ok {
			if cn, ok := obj.(*ComparableNum); ok {
				result = append(result, cn.n)
			}
		}
	}

	expected := []int{0, 1, 2, 3, 4}
	if !slices.Equal(result, expected) {
		t.Errorf("sorted nums = %v, want %v", result, expected)
	}
}

// -----------------------------------------------------------------------------
// Truthiness Tests
// -----------------------------------------------------------------------------

func TestValueTruthiness(t *testing.T) {
	tests := []struct {
		val  Value
		want bool
	}{
		{Undefined(), false},
		{None(), false},
		{FromBool(true), true},
		{FromBool(false), false},
		{FromInt(0), false},
		{FromInt(1), true},
		{FromFloat(0.0), false},
		{FromFloat(0.1), true},
		{FromString(""), false},
		{FromString("x"), true},
		{FromSlice(nil), false},
		{FromSlice([]Value{FromInt(1)}), true},
		{FromMap(nil), false},
		{FromMap(map[string]Value{"a": FromInt(1)}), true},
	}

	for _, tt := range tests {
		if got := tt.val.IsTrue(); got != tt.want {
			t.Errorf("%v.IsTrue() = %v, want %v", tt.val, got, tt.want)
		}
	}
}

// -----------------------------------------------------------------------------
// Value Kind Tests
// -----------------------------------------------------------------------------

func TestValueKind(t *testing.T) {
	tests := []struct {
		val  Value
		want ValueKind
	}{
		{Undefined(), KindUndefined},
		{None(), KindNone},
		{FromBool(true), KindBool},
		{FromInt(42), KindNumber},
		{FromFloat(3.14), KindNumber},
		{FromString("hello"), KindString},
		{FromBytes([]byte{1, 2, 3}), KindBytes},
		{FromSlice([]Value{FromInt(1)}), KindSeq},
		{FromMap(map[string]Value{"a": FromInt(1)}), KindMap},
	}

	for _, tt := range tests {
		if got := tt.val.Kind(); got != tt.want {
			t.Errorf("%v.Kind() = %v, want %v", tt.val, got, tt.want)
		}
	}
}

// Object kind tests
func TestObjectKind(t *testing.T) {
	// Seq object should have KindSeq
	seqObj := &SeqPoint{coords: [3]int{1, 2, 3}}
	if k := FromObject(seqObj).Kind(); k != KindSeq {
		t.Errorf("SeqObject.Kind() = %v, want KindSeq", k)
	}

	// Map object should have KindMap (default repr is Map for objects with Keys())
	// Actually our MapPoint doesn't implement ObjectWithRepr, so it falls back to Plain
	// Let's create a proper map object
	type mapObj struct {
		MapPoint
	}
	mo := &mapObj{MapPoint{x: 1, y: 2, z: 3}}
	// Plain objects with Keys() are still KindPlain unless they implement ObjectWithRepr
	if k := FromObject(mo).Kind(); k != KindPlain {
		t.Errorf("MapObject without ObjectRepr.Kind() = %v, want KindPlain", k)
	}
}

// -----------------------------------------------------------------------------
// Contains Tests
// -----------------------------------------------------------------------------

func TestValueContains(t *testing.T) {
	// String contains
	s := FromString("hello world")
	if !s.Contains(FromString("world")) {
		t.Error("'hello world' should contain 'world'")
	}
	if s.Contains(FromString("foo")) {
		t.Error("'hello world' should not contain 'foo'")
	}

	// Slice contains
	slice := FromSlice([]Value{FromInt(1), FromInt(2), FromInt(3)})
	if !slice.Contains(FromInt(2)) {
		t.Error("slice should contain 2")
	}
	if slice.Contains(FromInt(5)) {
		t.Error("slice should not contain 5")
	}

	// Map contains (checks keys)
	m := FromMap(map[string]Value{"a": FromInt(1), "b": FromInt(2)})
	if !m.Contains(FromString("a")) {
		t.Error("map should contain key 'a'")
	}
	if m.Contains(FromString("c")) {
		t.Error("map should not contain key 'c'")
	}
}

// -----------------------------------------------------------------------------
// Arithmetic Tests
// -----------------------------------------------------------------------------

func TestArithmetic(t *testing.T) {
	// Add
	if v, err := FromInt(1).Add(FromInt(2)); err != nil || v.String() != "3" {
		t.Errorf("1 + 2 = %v, err=%v", v, err)
	}

	// Sub
	if v, err := FromInt(5).Sub(FromInt(3)); err != nil || v.String() != "2" {
		t.Errorf("5 - 3 = %v, err=%v", v, err)
	}

	// Mul
	if v, err := FromInt(3).Mul(FromInt(4)); err != nil || v.String() != "12" {
		t.Errorf("3 * 4 = %v, err=%v", v, err)
	}

	// Div
	if v, err := FromFloat(10).Div(FromFloat(4)); err != nil || v.String() != "2.5" {
		t.Errorf("10 / 4 = %v, err=%v", v, err)
	}

	// FloorDiv
	if v, err := FromInt(10).FloorDiv(FromInt(3)); err != nil || v.String() != "3" {
		t.Errorf("10 // 3 = %v, err=%v", v, err)
	}

	// Rem
	if v, err := FromInt(10).Rem(FromInt(3)); err != nil || v.String() != "1" {
		t.Errorf("10 %% 3 = %v, err=%v", v, err)
	}

	// Pow
	if v, err := FromInt(2).Pow(FromInt(3)); err != nil || v.String() != "8" {
		t.Errorf("2 ** 3 = %v, err=%v", v, err)
	}

	// Neg
	if v, err := FromInt(42).Neg(); err != nil || v.String() != "-42" {
		t.Errorf("-42 = %v, err=%v", v, err)
	}

	// String concat
	if v, err := FromString("hello").Add(FromString(" world")); err != nil || v.String() != "hello world" {
		t.Errorf("'hello' + ' world' = %v, err=%v", v, err)
	}

	// String repetition
	if v, err := FromString("ab").Mul(FromInt(3)); err != nil || v.String() != "ababab" {
		t.Errorf("'ab' * 3 = %v, err=%v", v, err)
	}

	// Sequence concat
	s1 := FromSlice([]Value{FromInt(1), FromInt(2)})
	s2 := FromSlice([]Value{FromInt(3), FromInt(4)})
	if v, err := s1.Add(s2); err != nil {
		t.Errorf("sequence concat error: %v", err)
	} else {
		expected := "[1, 2, 3, 4]"
		if v.String() != expected {
			t.Errorf("sequence concat = %v, want %v", v.String(), expected)
		}
	}
}

// -----------------------------------------------------------------------------
// Clone Tests
// -----------------------------------------------------------------------------

func TestValueClone(t *testing.T) {
	// Clone slice - modifications to clone shouldn't affect original
	original := FromSlice([]Value{FromInt(1), FromInt(2), FromInt(3)})
	cloned := original.Clone()

	// Modify the cloned slice's underlying data
	if slice, ok := cloned.AsSlice(); ok {
		slice[0] = FromInt(99)
	}

	// Original should be unchanged
	if item := original.GetItem(FromInt(0)); item.String() != "1" {
		t.Errorf("original[0] = %v after clone modification, want 1", item)
	}
}

// -----------------------------------------------------------------------------
// Repr Tests
// -----------------------------------------------------------------------------

func TestValueRepr(t *testing.T) {
	tests := []struct {
		val  Value
		want string
	}{
		{Undefined(), "undefined"},
		{None(), "none"},
		{FromBool(true), "true"},
		{FromInt(42), "42"},
		{FromFloat(3.14), "3.14"},
		{FromString("hello"), `"hello"`},
		{FromSlice([]Value{FromInt(1), FromInt(2)}), "[1, 2]"},
	}

	for _, tt := range tests {
		if got := tt.val.Repr(); got != tt.want {
			t.Errorf("%v.Repr() = %q, want %q", tt.val, got, tt.want)
		}
	}
}

// -----------------------------------------------------------------------------
// MakeIterable Tests (ported from test_make_iterable)
// -----------------------------------------------------------------------------

func TestMakeIterableMultipleIterations(t *testing.T) {
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
		t.Errorf("first iteration = %v, want [0 1 2 3 4]", first)
	}

	// Second iteration should work again
	var second []int64
	for v := range IterateObject(val.data.(Object)) {
		i, _ := v.AsInt()
		second = append(second, i)
	}
	if !slices.Equal(second, []int64{0, 1, 2, 3, 4}) {
		t.Errorf("second iteration = %v, want [0 1 2 3 4]", second)
	}

	// Maker should be called twice (once per iteration)
	if callCount != 2 {
		t.Errorf("maker call count = %d, want 2", callCount)
	}
}

// -----------------------------------------------------------------------------
// MakeOneShotIterator Tests (ported from test_one_shot_iterator)
// -----------------------------------------------------------------------------

func TestOneShotIteratorBasic(t *testing.T) {
	val := MakeOneShotIterator(func(yield func(Value) bool) {
		for i := 0; i < 10; i++ {
			if !yield(FromInt(int64(i))) {
				return
			}
		}
	})

	// String representation
	if s := val.String(); s != "<iterator>" {
		t.Errorf("String() = %q, want '<iterator>'", s)
	}

	// First iteration consumes all
	var items []int64
	for v := range IterateObject(val.data.(Object)) {
		i, _ := v.AsInt()
		items = append(items, i)
	}
	if len(items) != 10 {
		t.Errorf("got %d items, want 10", len(items))
	}

	// Second iteration yields nothing
	var second []int64
	for v := range IterateObject(val.data.(Object)) {
		i, _ := v.AsInt()
		second = append(second, i)
	}
	if len(second) != 0 {
		t.Errorf("second iteration got %d items, want 0", len(second))
	}
}

// -----------------------------------------------------------------------------
// Negative Index Tests
// -----------------------------------------------------------------------------

func TestNegativeIndex(t *testing.T) {
	slice := FromSlice([]Value{FromInt(1), FromInt(2), FromInt(3)})

	// Negative index from end
	if v := slice.GetItem(FromInt(-1)); v.String() != "3" {
		t.Errorf("slice[-1] = %v, want 3", v)
	}
	if v := slice.GetItem(FromInt(-2)); v.String() != "2" {
		t.Errorf("slice[-2] = %v, want 2", v)
	}
	if v := slice.GetItem(FromInt(-3)); v.String() != "1" {
		t.Errorf("slice[-3] = %v, want 1", v)
	}

	// Out of bounds negative index
	if v := slice.GetItem(FromInt(-4)); !v.IsUndefined() {
		t.Errorf("slice[-4] = %v, want undefined", v)
	}
}

// -----------------------------------------------------------------------------
// String Index Tests
// -----------------------------------------------------------------------------

func TestStringIndex(t *testing.T) {
	s := FromString("hello")

	if v := s.GetItem(FromInt(0)); v.String() != "h" {
		t.Errorf("s[0] = %v, want 'h'", v)
	}
	if v := s.GetItem(FromInt(-1)); v.String() != "o" {
		t.Errorf("s[-1] = %v, want 'o'", v)
	}

	// Unicode string
	u := FromString("héllo")
	if v := u.GetItem(FromInt(1)); v.String() != "é" {
		t.Errorf("u[1] = %v, want 'é'", v)
	}
}

// -----------------------------------------------------------------------------
// Length Tests
// -----------------------------------------------------------------------------

func TestValueLen(t *testing.T) {
	tests := []struct {
		val     Value
		wantLen int
		wantOk  bool
	}{
		{FromString("hello"), 5, true},
		{FromString("héllo"), 5, true}, // Unicode length
		{FromSlice([]Value{FromInt(1), FromInt(2)}), 2, true},
		{FromMap(map[string]Value{"a": FromInt(1)}), 1, true},
		{FromBytes([]byte{1, 2, 3}), 3, true},
		{FromInt(42), 0, false},
		{FromBool(true), 0, false},
	}

	for _, tt := range tests {
		l, ok := tt.val.Len()
		if ok != tt.wantOk {
			t.Errorf("%v.Len() ok = %v, want %v", tt.val, ok, tt.wantOk)
		}
		if ok && l != tt.wantLen {
			t.Errorf("%v.Len() = %d, want %d", tt.val, l, tt.wantLen)
		}
	}
}

// -----------------------------------------------------------------------------
// SameAs Tests (identity comparison)
// -----------------------------------------------------------------------------

func TestValueSameAs(t *testing.T) {
	// Same slice literal is same as itself
	slice := FromSlice([]Value{FromInt(1)})
	if !slice.SameAs(slice) {
		t.Error("slice should be same as itself")
	}

	// Two different slice literals are not same
	slice2 := FromSlice([]Value{FromInt(1)})
	if slice.SameAs(slice2) {
		t.Error("different slices should not be same")
	}

	// Primitives with same value are same
	i1 := FromInt(42)
	i2 := FromInt(42)
	if !i1.SameAs(i2) {
		t.Error("same int values should be same")
	}

	// Different primitive values are not same
	i3 := FromInt(43)
	if i1.SameAs(i3) {
		t.Error("different int values should not be same")
	}
}
