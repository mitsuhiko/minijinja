package value

import (
	"math"
	"math/big"
	"testing"
)

func TestMapKeysAreValues(t *testing.T) {
	m := NewMap()
	m.Set(FromString("b"), FromInt(1))
	m.Set(FromInt(1), FromString("int"))
	m.Set(FromString("1"), FromString("str"))
	m.Set(None(), FromString("none"))
	m.Set(FromTuple([]Value{FromInt(1), FromInt(2)}), FromString("tuple"))
	// equal keys replace the value but keep the original key and position
	m.Set(True(), FromString("bool"))
	m.Set(FromFloat(1.0), FromString("float"))

	if got := FromValueMap(m).Repr(); got != "{'b': 1, 1: 'float', '1': 'str', None: 'none', (1, 2): 'tuple'}" {
		t.Fatalf("unexpected repr %s", got)
	}
	for _, tc := range []struct {
		key  Value
		want string
	}{
		{FromInt(1), "float"},
		{True(), "float"},
		{FromFloat(1), "float"},
		{FromBigInt(big.NewInt(1)), "float"},
		{FromString("1"), "str"},
		{FromSafeString("b"), "1"},
		{None(), "none"},
		{FromTuple([]Value{FromInt(1), FromFloat(2)}), "tuple"},
	} {
		got, ok := m.Get(tc.key)
		if !ok || got.String() != tc.want {
			t.Errorf("Get(%s) = %s, %v; want %s", tc.key.Repr(), got.Repr(), ok, tc.want)
		}
	}
	for _, key := range []Value{False(), FromFloat(1.5), Undefined(), FromString("a"), FromSlice([]Value{FromInt(1), FromInt(2)})} {
		if _, ok := m.Get(key); ok {
			t.Errorf("Get(%s) unexpectedly found a value", key.Repr())
		}
	}
	if m.Len() != 5 {
		t.Errorf("unexpected length %d", m.Len())
	}
}

func TestMapNumberKeys(t *testing.T) {
	big1 := new(big.Int).Lsh(big.NewInt(1), 70)
	big2 := new(big.Int).Add(big1, big.NewInt(1))
	m := NewMap()
	m.Set(FromBigInt(big1), FromString("pow"))
	m.Set(FromBigInt(big2), FromString("pow+1"))
	m.Set(FromFloat(0.5), FromString("half"))
	m.Set(FromFloat(math.Copysign(0, -1)), FromString("zero"))

	if v, ok := m.Get(FromFloat(math.Ldexp(1, 70))); !ok || v.String() != "pow" {
		t.Errorf("float key did not find the equal big integer: %s %v", v.Repr(), ok)
	}
	if v, ok := m.Get(FromBigInt(new(big.Int).Set(big2))); !ok || v.String() != "pow+1" {
		t.Errorf("big integer key not found: %s %v", v.Repr(), ok)
	}
	if v, ok := m.Get(FromInt(0)); !ok || v.String() != "zero" {
		t.Errorf("-0.0 and 0 are not the same key: %s %v", v.Repr(), ok)
	}
	if v, ok := m.Get(FromFloat(0.5)); !ok || v.String() != "half" {
		t.Errorf("float key not found: %s %v", v.Repr(), ok)
	}
}

func TestMapValueAPI(t *testing.T) {
	m := NewMap()
	m.Set(FromInt(2), FromString("a"))
	m.Set(FromInt(1), FromString("b"))
	val := FromValueMap(m)

	if val.Kind() != KindMap || !val.IsTrue() {
		t.Fatalf("unexpected kind or truthiness")
	}
	if n, _ := val.Len(); n != 2 {
		t.Errorf("unexpected length %d", n)
	}
	if got := FromSlice(val.Iter()).Repr(); got != "[2, 1]" {
		t.Errorf("unexpected iteration order %s", got)
	}
	if got := val.GetItem(FromFloat(1)); got.String() != "b" {
		t.Errorf("unexpected item %s", got.Repr())
	}
	if !val.Contains(True()) || val.Contains(FromString("1")) {
		t.Errorf("unexpected containment")
	}
	if !val.SameAs(val) || val.SameAs(val.Clone()) || !val.Equal(val.Clone()) {
		t.Errorf("unexpected identity")
	}

	// AsMap converts keys to strings
	sm, ok := val.AsMap()
	if !ok || len(sm) != 2 || sm["1"].String() != "b" || sm["2"].String() != "a" {
		t.Errorf("unexpected AsMap result %v", sm)
	}

	// Go maps convert into maps with sorted keys
	vm, ok := FromMap(map[string]Value{"b": FromInt(1), "a": FromInt(2)}).AsValueMap()
	if !ok || FromSlice(vm.Keys()).Repr() != "['a', 'b']" {
		t.Errorf("unexpected AsValueMap result")
	}
	if _, ok := FromInt(1).AsValueMap(); ok {
		t.Errorf("integers are not maps")
	}

	// maps with and without value keys compare equal
	if !FromMap(map[string]Value{"a": FromInt(1)}).Equal(FromValueMap(func() *Map {
		m := NewMap()
		m.Set(FromString("a"), FromInt(1))
		return m
	}())) {
		t.Errorf("string maps should equal value maps with the same items")
	}
}

func TestFromAnyMapKeys(t *testing.T) {
	val := FromAny(map[int]string{10: "ten", 9: "nine", -1: "minus one"})
	if got := val.Repr(); got != "{-1: 'minus one', 9: 'nine', 10: 'ten'}" {
		t.Errorf("unexpected repr %s", got)
	}
	if got := val.GetItem(FromInt(9)); got.String() != "nine" {
		t.Errorf("unexpected item %s", got.Repr())
	}
	val = FromAny(map[any]any{"a": 1, 2: "b", nil: true})
	if got := val.Repr(); got != "{None: True, 2: 'b', 'a': 1}" {
		t.Errorf("unexpected repr %s", got)
	}
	// string keys stay a Go map
	if _, ok := FromAny(map[string]int{"a": 1}).Raw().(map[string]Value); !ok {
		t.Errorf("maps with string keys should stay Go maps")
	}
	// string keys that look like integers are strings
	if got := FromAny(map[string]int{"1": 1}).Repr(); got != "{'1': 1}" {
		t.Errorf("unexpected repr %s", got)
	}
}

func TestCompareMapsByValueKeys(t *testing.T) {
	m := func(k Value) Value {
		rv := NewMap()
		rv.Set(k, FromInt(1))
		return FromValueMap(rv)
	}
	if cmp, ok := m(FromInt(9)).Compare(m(FromInt(10))); !ok || cmp >= 0 {
		t.Errorf("{9: 1} should sort before {10: 1}")
	}
	if cmp, ok := m(FromInt(10)).Compare(m(FromString("a"))); !ok || cmp >= 0 {
		t.Errorf("numbers should sort before strings")
	}
}
