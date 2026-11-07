package value

import (
	"encoding/binary"
	"iter"
	"math"
	"math/big"
	"sort"
)

// Map is an insertion ordered map with [Value] keys.
//
// This is the map type that templates create for dict literals ({1: 'a'}),
// the dict() function and filters that build maps.  Unlike a Go map with
// string keys it can hold keys of any kind, and keys are compared like
// [Value.Equal] compares values, so 1, 1.0 and true are the same key, while
// 1 and "1" are different keys.  This matches MiniJinja for Rust (with the
// preserve_order feature) and Python dicts in Jinja2.
//
// Setting a key that already exists replaces the value but keeps the
// position and the original key, so {true: 'a', 1: 'b'} is {True: 'b'}.
//
// A Map is not safe for concurrent mutation.  Maps created by templates are
// not modified after they were created.
type Map struct {
	keys   []Value
	values []Value
	// index maps the canonical form of a key to its position.
	index map[mapKey]int
	// opaque holds the positions of keys that have no canonical form
	// (objects, NaN and sequences or maps containing them).  They are
	// compared with Equal.
	opaque []int
}

// NewMap creates an empty [Map].
func NewMap() *Map {
	return newMapWithCapacity(0)
}

func newMapWithCapacity(capacity int) *Map {
	if capacity < 0 {
		capacity = 0
	}
	// Do not trust huge size hints.
	if capacity > 1024 {
		capacity = 1024
	}
	return &Map{
		keys:   make([]Value, 0, capacity),
		values: make([]Value, 0, capacity),
		index:  make(map[mapKey]int, capacity),
	}
}

// FromValueMap creates a map Value from a [Map].
//
// The value references the map, so later changes to the map are visible
// through the value.
//
// Example usage:
//
//	m := value.NewMap()
//	m.Set(value.FromInt(1), value.FromString("one"))
//	m.Set(value.FromString("two"), value.FromInt(2))
//	val := value.FromValueMap(m)
//	// In template: {{ val }}  -> {1: 'one', 'two': 2}
//	// In template: {{ val[1] }}  -> one
func FromValueMap(m *Map) Value {
	if m == nil {
		m = NewMap()
	}
	return Value{data: m}
}

// Len returns the number of entries in the map.
func (m *Map) Len() int {
	if m == nil {
		return 0
	}
	return len(m.keys)
}

// Set sets the value of a key.  If the key already exists, its value is
// replaced in place and the original key is kept.
func (m *Map) Set(key, val Value) {
	if idx, ok := m.find(key); ok {
		m.values[idx] = val
		return
	}
	idx := len(m.keys)
	m.keys = append(m.keys, key)
	m.values = append(m.values, val)
	if mk, ok := canonicalMapKey(key); ok {
		if m.index == nil {
			m.index = make(map[mapKey]int)
		}
		m.index[mk] = idx
	} else {
		m.opaque = append(m.opaque, idx)
	}
}

// Get returns the value of a key and whether the key exists.
func (m *Map) Get(key Value) (Value, bool) {
	if idx, ok := m.find(key); ok {
		return m.values[idx], true
	}
	return Undefined(), false
}

// Keys returns the keys of the map in insertion order.
func (m *Map) Keys() []Value {
	if m == nil {
		return nil
	}
	return append([]Value(nil), m.keys...)
}

// Values returns the values of the map in insertion order.
func (m *Map) Values() []Value {
	if m == nil {
		return nil
	}
	return append([]Value(nil), m.values...)
}

// All returns an iterator over the key-value pairs in insertion order.
func (m *Map) All() iter.Seq2[Value, Value] {
	return func(yield func(Value, Value) bool) {
		if m == nil {
			return
		}
		for i := range m.keys {
			if !yield(m.keys[i], m.values[i]) {
				return
			}
		}
	}
}

// Clone returns a shallow copy of the map.
func (m *Map) Clone() *Map {
	rv := newMapWithCapacity(m.Len())
	if m == nil {
		return rv
	}
	rv.keys = append(rv.keys, m.keys...)
	rv.values = append(rv.values, m.values...)
	for k, idx := range m.index {
		rv.index[k] = idx
	}
	rv.opaque = append(rv.opaque, m.opaque...)
	return rv
}

func (m *Map) find(key Value) (int, bool) {
	if m == nil {
		return 0, false
	}
	mk, ok := canonicalMapKey(key)
	if !ok {
		// Opaque keys (such as objects) can be equal to any sequence or
		// map key, so they are compared with all keys.
		for idx, k := range m.keys {
			if k.Equal(key) {
				return idx, true
			}
		}
		return 0, false
	}
	if idx, found := m.index[mk]; found {
		return idx, true
	}
	// Primitive keys are never equal to opaque keys, but sequences and
	// maps can be equal to objects.
	if mk.kind == mapKeyComposite {
		for _, idx := range m.opaque {
			if m.keys[idx].Equal(key) {
				return idx, true
			}
		}
	}
	return 0, false
}

// getString looks up a string key without creating a Value for it.
func (m *Map) getString(key string) (Value, bool) {
	if m == nil {
		return Undefined(), false
	}
	if idx, ok := m.index[mapKey{kind: mapKeyString, s: key}]; ok {
		return m.values[idx], true
	}
	return Undefined(), false
}

// mapKeyKind is the kind of a canonical map key.
type mapKeyKind uint8

const (
	mapKeyUndefined mapKeyKind = iota
	mapKeyNone
	mapKeyInt
	mapKeyFloat
	mapKeyBigInt
	mapKeyString
	mapKeyBytes
	// mapKeyComposite is a sequence or map encoded with encodeMapKey.
	mapKeyComposite
)

// mapKey is the canonical, comparable form of a key.  Two keys have the
// same canonical form exactly if they are equal values: booleans and
// integral floats become integers, so true, 1 and 1.0 are one key.
// Sequences and maps are encoded from the canonical forms of their items.
type mapKey struct {
	kind mapKeyKind
	n    uint64
	s    string
}

// canonicalMapKey returns the canonical form of a key.  It returns false for
// keys without one: objects, which can implement their own equality, NaN,
// which is not equal to itself, and sequences and maps containing them.
func canonicalMapKey(v Value) (mapKey, bool) {
	switch d := v.data.(type) {
	case undefinedType:
		return mapKey{kind: mapKeyUndefined}, true
	case noneType:
		return mapKey{kind: mapKeyNone}, true
	case bool:
		return mapKey{kind: mapKeyInt, n: uint64(boolToInt(d))}, true
	case int64:
		return mapKey{kind: mapKeyInt, n: uint64(d)}, true
	case float64:
		return floatMapKey(d)
	case bigIntValue:
		if d.Int.IsInt64() {
			return mapKey{kind: mapKeyInt, n: uint64(d.Int.Int64())}, true
		}
		// A big integer that a float represents exactly is equal to that
		// float, so it needs to have the same canonical form.
		if f, acc := new(big.Float).SetInt(d.Int).Float64(); acc == big.Exact {
			return floatMapKey(f)
		}
		return mapKey{kind: mapKeyBigInt, s: d.Int.String()}, true
	case string:
		return mapKey{kind: mapKeyString, s: d}, true
	case safeString:
		return mapKey{kind: mapKeyString, s: string(d)}, true
	case []byte:
		return mapKey{kind: mapKeyBytes, s: string(d)}, true
	case []Value, tupleValue, *Iterator, map[string]Value, *Map:
		buf, ok := encodeMapKey(nil, v)
		if !ok {
			return mapKey{}, false
		}
		return mapKey{kind: mapKeyComposite, s: string(buf)}, true
	}
	return mapKey{}, false
}

func floatMapKey(f float64) (mapKey, bool) {
	if math.IsNaN(f) {
		return mapKey{}, false
	}
	if f == math.Trunc(f) && f >= -(1<<63) && f < 1<<63 {
		return mapKey{kind: mapKeyInt, n: uint64(int64(f))}, true
	}
	return mapKey{kind: mapKeyFloat, n: math.Float64bits(f)}, true
}

// encodeMapKey appends an encoding of a key to buf that is equal for two
// keys exactly if they are equal values.  Like Equal, lists and iterables
// with the same items are equal, but tuples are only equal to tuples, and
// maps are equal if they have the same entries in any order.
func encodeMapKey(buf []byte, v Value) ([]byte, bool) {
	encodeItems := func(tag byte, items []Value) bool {
		buf = append(buf, tag)
		buf = binary.AppendUvarint(buf, uint64(len(items)))
		for _, item := range items {
			var ok bool
			if buf, ok = encodeMapKey(buf, item); !ok {
				return false
			}
		}
		return true
	}
	switch d := v.data.(type) {
	case []Value:
		return buf, encodeItems('l', d)
	case *Iterator:
		return buf, encodeItems('l', d.items)
	case tupleValue:
		return buf, encodeItems('t', d)
	case map[string]Value, *Map:
		m, _ := v.AsValueMap()
		entries := make([]string, 0, m.Len())
		for i, k := range m.keys {
			entry, ok := encodeMapKey(nil, k)
			if !ok {
				return nil, false
			}
			if entry, ok = encodeMapKey(entry, m.values[i]); !ok {
				return nil, false
			}
			entries = append(entries, string(entry))
		}
		sort.Strings(entries)
		buf = append(buf, 'm')
		buf = binary.AppendUvarint(buf, uint64(len(entries)))
		for _, entry := range entries {
			buf = binary.AppendUvarint(buf, uint64(len(entry)))
			buf = append(buf, entry...)
		}
		return buf, true
	}
	mk, ok := canonicalMapKey(v)
	if !ok {
		return nil, false
	}
	buf = append(buf, 'p', byte(mk.kind))
	buf = binary.LittleEndian.AppendUint64(buf, mk.n)
	buf = binary.AppendUvarint(buf, uint64(len(mk.s)))
	return append(buf, mk.s...), true
}

// mapFromStringMap converts a Go map into a Map with sorted keys.
func mapFromStringMap(m map[string]Value) *Map {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	rv := newMapWithCapacity(len(keys))
	for _, k := range keys {
		rv.Set(FromString(k), m[k])
	}
	return rv
}

// stringMap converts a Map into a Go map with string keys.  Keys that are
// not strings are converted to their string representation.
func (m *Map) stringMap() map[string]Value {
	rv := make(map[string]Value, m.Len())
	for i, k := range m.keys {
		s, ok := k.AsString()
		if !ok {
			s = k.String()
		}
		rv[s] = m.values[i]
	}
	return rv
}

// sortPairs sorts key-value pairs by their keys like MiniJinja for Rust
// orders values.
func sortPairs(pairs [][2]Value) {
	sort.SliceStable(pairs, func(i, j int) bool {
		cmp, ok := pairs[i][0].Compare(pairs[j][0])
		return ok && cmp < 0
	})
}

// sortedPairs returns the key-value pairs of the map sorted by key.
func (m *Map) sortedPairs() [][2]Value {
	pairs := make([][2]Value, m.Len())
	for i := range pairs {
		pairs[i] = [2]Value{m.keys[i], m.values[i]}
	}
	sortPairs(pairs)
	return pairs
}
