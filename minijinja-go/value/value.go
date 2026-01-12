// Package value provides a dynamic value type for the template engine.
package value

import (
	"fmt"
	"math"
	"reflect"
	"sort"
	"strings"
)

// ValueKind describes the type of a Value.
type ValueKind int

const (
	KindUndefined ValueKind = iota
	KindNone
	KindBool
	KindNumber
	KindString
	KindBytes
	KindSeq
	KindMap
	KindIterable
	KindPlain
	KindInvalid
)

func (k ValueKind) String() string {
	switch k {
	case KindUndefined:
		return "undefined"
	case KindNone:
		return "none"
	case KindBool:
		return "bool"
	case KindNumber:
		return "number"
	case KindString:
		return "string"
	case KindBytes:
		return "bytes"
	case KindSeq:
		return "sequence"
	case KindMap:
		return "map"
	case KindIterable:
		return "iterator"
	case KindPlain:
		return "plain object"
	case KindInvalid:
		return "invalid value"
	default:
		return "unknown"
	}
}

// Value represents a dynamically typed value in the template engine.
type Value struct {
	data any
}

// internal marker types
type undefinedType struct{}
type noneType struct{}

var (
	undefinedVal = undefinedType{}
	noneVal      = noneType{}
)

// Undefined returns the undefined value.
func Undefined() Value {
	return Value{data: undefinedVal}
}

// None returns the none value.
func None() Value {
	return Value{data: noneVal}
}

// True returns a true boolean value.
func True() Value {
	return Value{data: true}
}

// False returns a false boolean value.
func False() Value {
	return Value{data: false}
}

// FromBool creates a Value from a bool.
func FromBool(v bool) Value {
	return Value{data: v}
}

// FromInt creates a Value from an int64.
func FromInt(v int64) Value {
	return Value{data: v}
}

// FromFloat creates a Value from a float64.
func FromFloat(v float64) Value {
	return Value{data: v}
}

// FromString creates a Value from a string.
func FromString(v string) Value {
	return Value{data: v}
}

// FromSafeString creates a Value from a safe (pre-escaped) string.
func FromSafeString(v string) Value {
	return Value{data: safeString(v)}
}

// safeString is a string that should not be escaped.
type safeString string

// FromBytes creates a Value from a byte slice.
func FromBytes(v []byte) Value {
	return Value{data: v}
}

// FromSlice creates a Value from a slice of Values.
func FromSlice(v []Value) Value {
	return Value{data: v}
}

// FromMap creates a Value from a map of string to Value.
func FromMap(v map[string]Value) Value {
	return Value{data: v}
}

// FromAny creates a Value from any Go value using reflection.
func FromAny(v any) Value {
	if v == nil {
		return None()
	}

	// Already a Value
	if val, ok := v.(Value); ok {
		return val
	}

	rv := reflect.ValueOf(v)
	return fromReflectValue(rv)
}

func fromReflectValue(rv reflect.Value) Value {
	if !rv.IsValid() {
		return None()
	}

	switch rv.Kind() {
	case reflect.Bool:
		return FromBool(rv.Bool())
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return FromInt(rv.Int())
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		return FromInt(int64(rv.Uint()))
	case reflect.Float32, reflect.Float64:
		return FromFloat(rv.Float())
	case reflect.String:
		return FromString(rv.String())
	case reflect.Slice:
		if rv.Type().Elem().Kind() == reflect.Uint8 {
			return FromBytes(rv.Bytes())
		}
		slice := make([]Value, rv.Len())
		for i := 0; i < rv.Len(); i++ {
			slice[i] = fromReflectValue(rv.Index(i))
		}
		return FromSlice(slice)
	case reflect.Array:
		slice := make([]Value, rv.Len())
		for i := 0; i < rv.Len(); i++ {
			slice[i] = fromReflectValue(rv.Index(i))
		}
		return FromSlice(slice)
	case reflect.Map:
		m := make(map[string]Value)
		iter := rv.MapRange()
		for iter.Next() {
			k := iter.Key()
			var key string
			if k.Kind() == reflect.String {
				key = k.String()
			} else {
				key = fmt.Sprintf("%v", k.Interface())
			}
			m[key] = fromReflectValue(iter.Value())
		}
		return FromMap(m)
	case reflect.Struct:
		return fromStruct(rv)
	case reflect.Ptr, reflect.Interface:
		if rv.IsNil() {
			return None()
		}
		return fromReflectValue(rv.Elem())
	default:
		// Wrap unknown types as-is for potential method calls
		return Value{data: rv.Interface()}
	}
}

func fromStruct(rv reflect.Value) Value {
	t := rv.Type()
	m := make(map[string]Value)
	for i := 0; i < t.NumField(); i++ {
		field := t.Field(i)
		if !field.IsExported() {
			continue
		}
		name := field.Name
		// Check for json tag
		if tag := field.Tag.Get("json"); tag != "" {
			parts := strings.Split(tag, ",")
			if parts[0] != "" && parts[0] != "-" {
				name = parts[0]
			} else if parts[0] == "-" {
				continue
			}
		}
		m[name] = fromReflectValue(rv.Field(i))
	}
	return FromMap(m)
}

// Kind returns the kind of value.
func (v Value) Kind() ValueKind {
	switch v.data.(type) {
	case undefinedType:
		return KindUndefined
	case noneType:
		return KindNone
	case bool:
		return KindBool
	case int64, float64:
		return KindNumber
	case string, safeString:
		return KindString
	case []byte:
		return KindBytes
	case []Value:
		return KindSeq
	case map[string]Value:
		return KindMap
	default:
		return KindPlain
	}
}

// IsUndefined returns true if the value is undefined.
func (v Value) IsUndefined() bool {
	_, ok := v.data.(undefinedType)
	return ok
}

// IsNone returns true if the value is none.
func (v Value) IsNone() bool {
	_, ok := v.data.(noneType)
	return ok
}

// IsTrue returns the truthiness of the value.
func (v Value) IsTrue() bool {
	switch d := v.data.(type) {
	case undefinedType, noneType:
		return false
	case bool:
		return d
	case int64:
		return d != 0
	case float64:
		return d != 0 && !math.IsNaN(d)
	case string:
		return d != ""
	case safeString:
		return d != ""
	case []byte:
		return len(d) > 0
	case []Value:
		return len(d) > 0
	case map[string]Value:
		return len(d) > 0
	default:
		return true
	}
}

// String returns a string representation of the value.
func (v Value) String() string {
	switch d := v.data.(type) {
	case undefinedType:
		return ""
	case noneType:
		return "none"
	case bool:
		if d {
			return "true"
		}
		return "false"
	case int64:
		return fmt.Sprintf("%d", d)
	case float64:
		// Match Jinja2's float formatting
		if d == math.Trunc(d) && math.Abs(d) < 1e15 {
			return fmt.Sprintf("%.1f", d)
		}
		return fmt.Sprintf("%g", d)
	case string:
		return d
	case safeString:
		return string(d)
	case []byte:
		return string(d)
	case []Value:
		var parts []string
		for _, item := range d {
			parts = append(parts, item.Repr())
		}
		return "[" + strings.Join(parts, ", ") + "]"
	case map[string]Value:
		var parts []string
		keys := make([]string, 0, len(d))
		for k := range d {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			parts = append(parts, fmt.Sprintf("%q: %s", k, d[k].Repr()))
		}
		return "{" + strings.Join(parts, ", ") + "}"
	default:
		return fmt.Sprintf("%v", d)
	}
}

// Repr returns a debug representation of the value.
func (v Value) Repr() string {
	switch d := v.data.(type) {
	case undefinedType:
		return "undefined"
	case noneType:
		return "none"
	case bool:
		if d {
			return "true"
		}
		return "false"
	case int64:
		return fmt.Sprintf("%d", d)
	case float64:
		if d == math.Trunc(d) && math.Abs(d) < 1e15 {
			return fmt.Sprintf("%.1f", d)
		}
		return fmt.Sprintf("%g", d)
	case string:
		return fmt.Sprintf("%q", d)
	case safeString:
		return fmt.Sprintf("%q", string(d))
	case []byte:
		return fmt.Sprintf("b%q", d)
	case []Value:
		var parts []string
		for _, item := range d {
			parts = append(parts, item.Repr())
		}
		return "[" + strings.Join(parts, ", ") + "]"
	case map[string]Value:
		var parts []string
		keys := make([]string, 0, len(d))
		for k := range d {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			parts = append(parts, fmt.Sprintf("%q: %s", k, d[k].Repr()))
		}
		return "{" + strings.Join(parts, ", ") + "}"
	default:
		return fmt.Sprintf("%v", d)
	}
}

// IsSafe returns true if this is a safe string.
func (v Value) IsSafe() bool {
	_, ok := v.data.(safeString)
	return ok
}

// AsString returns the string value if it is one.
func (v Value) AsString() (string, bool) {
	switch d := v.data.(type) {
	case string:
		return d, true
	case safeString:
		return string(d), true
	default:
		return "", false
	}
}

// AsInt returns the integer value if it is one.
func (v Value) AsInt() (int64, bool) {
	switch d := v.data.(type) {
	case int64:
		return d, true
	case float64:
		if d == math.Trunc(d) {
			return int64(d), true
		}
		return 0, false
	default:
		return 0, false
	}
}

// AsFloat returns the float value if it is numeric.
func (v Value) AsFloat() (float64, bool) {
	switch d := v.data.(type) {
	case int64:
		return float64(d), true
	case float64:
		return d, true
	default:
		return 0, false
	}
}

// AsBool returns the boolean value if it is one.
func (v Value) AsBool() (bool, bool) {
	if b, ok := v.data.(bool); ok {
		return b, true
	}
	return false, false
}

// AsSlice returns the slice if it is one.
func (v Value) AsSlice() ([]Value, bool) {
	if s, ok := v.data.([]Value); ok {
		return s, true
	}
	return nil, false
}

// AsMap returns the map if it is one.
func (v Value) AsMap() (map[string]Value, bool) {
	if m, ok := v.data.(map[string]Value); ok {
		return m, true
	}
	return nil, false
}

// Len returns the length of the value if it has one.
func (v Value) Len() (int, bool) {
	switch d := v.data.(type) {
	case string:
		return len([]rune(d)), true
	case safeString:
		return len([]rune(d)), true
	case []byte:
		return len(d), true
	case []Value:
		return len(d), true
	case map[string]Value:
		return len(d), true
	default:
		return 0, false
	}
}

// GetItem gets an item by key (string or integer index).
func (v Value) GetItem(key Value) Value {
	switch d := v.data.(type) {
	case []Value:
		if idx, ok := key.AsInt(); ok {
			if idx < 0 {
				idx = int64(len(d)) + idx
			}
			if idx >= 0 && idx < int64(len(d)) {
				return d[idx]
			}
		}
	case map[string]Value:
		if s, ok := key.AsString(); ok {
			if val, exists := d[s]; exists {
				return val
			}
		}
	case string:
		if idx, ok := key.AsInt(); ok {
			runes := []rune(d)
			if idx < 0 {
				idx = int64(len(runes)) + idx
			}
			if idx >= 0 && idx < int64(len(runes)) {
				return FromString(string(runes[idx]))
			}
		}
	case safeString:
		if idx, ok := key.AsInt(); ok {
			runes := []rune(d)
			if idx < 0 {
				idx = int64(len(runes)) + idx
			}
			if idx >= 0 && idx < int64(len(runes)) {
				return FromString(string(runes[idx]))
			}
		}
	}
	return Undefined()
}

// GetAttr gets an attribute by name.
func (v Value) GetAttr(name string) Value {
	switch d := v.data.(type) {
	case map[string]Value:
		if val, ok := d[name]; ok {
			return val
		}
	}
	return Undefined()
}

// Iter returns an iterator over the value's items.
func (v Value) Iter() []Value {
	switch d := v.data.(type) {
	case []Value:
		return d
	case map[string]Value:
		keys := make([]string, 0, len(d))
		for k := range d {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		result := make([]Value, len(keys))
		for i, k := range keys {
			result[i] = FromString(k)
		}
		return result
	case string:
		runes := []rune(d)
		result := make([]Value, len(runes))
		for i, r := range runes {
			result[i] = FromString(string(r))
		}
		return result
	case safeString:
		runes := []rune(d)
		result := make([]Value, len(runes))
		for i, r := range runes {
			result[i] = FromString(string(r))
		}
		return result
	default:
		return nil
	}
}

// Clone creates a copy of the value.
func (v Value) Clone() Value {
	switch d := v.data.(type) {
	case []Value:
		newSlice := make([]Value, len(d))
		copy(newSlice, d)
		return Value{data: newSlice}
	case map[string]Value:
		newMap := make(map[string]Value, len(d))
		for k, val := range d {
			newMap[k] = val
		}
		return Value{data: newMap}
	default:
		return v // Immutable types can be shared
	}
}

// Raw returns the underlying Go value.
func (v Value) Raw() any {
	return v.data
}
