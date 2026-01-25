// Package value provides a dynamic value type for the template engine.
//
// The value package implements MiniJinja's dynamic type system, which allows
// templates to work with values of different types (strings, numbers, lists,
// maps, etc.) without compile-time type information.
//
// # Core Concepts
//
// The Value type is the central type in this package. It can hold any Go value
// and provides methods for type checking, conversion, and operations. Values
// can be created from Go primitives using constructor functions like FromInt,
// FromString, FromSlice, etc.
//
// # Type System
//
// MiniJinja supports the following value kinds:
//   - Undefined: Represents a missing or undefined value
//   - None: Represents null/nil
//   - Bool: Boolean true/false
//   - Number: Integers and floating-point numbers
//   - String: UTF-8 text strings (safe or unsafe for auto-escaping)
//   - Bytes: Binary data
//   - Seq: Ordered sequences (arrays/slices)
//   - Map: Key-value mappings (dictionaries/objects)
//   - Iterable: Lazy iterators
//   - Callable: Functions and macros
//   - Plain: Custom objects with attribute access
//
// # Example Usage
//
//	// Create values from Go types
//	name := value.FromString("World")
//	count := value.FromInt(42)
//	items := value.FromSlice([]value.Value{
//	    value.FromString("apple"),
//	    value.FromString("banana"),
//	})
//
//	// Create a map/dict value
//	context := value.FromMap(map[string]value.Value{
//	    "name":  name,
//	    "count": count,
//	    "items": items,
//	})
//
//	// Type checking
//	if name.Kind() == value.KindString {
//	    fmt.Println("It's a string!")
//	}
//
//	// Conversion
//	if s, ok := name.AsString(); ok {
//	    fmt.Println("String value:", s)
//	}
package value

import (
	"fmt"
	"math"
	"math/big"
	"reflect"
	"sort"
	"strconv"
	"strings"
)

// Callable is an interface for callable objects like functions and macros.
//
// Types that implement Callable can be invoked from templates using the
// call syntax: {{ my_function(arg1, arg2) }} or {{ my_macro(key=value) }}.
//
// Example implementation:
//
//	type myCallable struct{}
//
//	func (m *myCallable) Call(state State, args []Value, kwargs map[string]Value) (Value, error) {
//	    // Process positional arguments
//	    if len(args) > 0 {
//	        fmt.Println("First arg:", args[0].String())
//	    }
//	    // Process keyword arguments
//	    if name, ok := kwargs["name"]; ok {
//	        fmt.Println("Name:", name.String())
//	    }
//	    return FromString("result"), nil
//	}
type Callable interface {
	// Call invokes the callable with positional and keyword arguments.
	//
	// The state provides access to the template rendering context.
	// The args slice contains positional arguments in order.
	// The kwargs map contains keyword arguments by name.
	//
	// Returns the result value and any error that occurred.
	Call(state State, args []Value, kwargs map[string]Value) (Value, error)
}

// Object is an interface for custom objects with attribute access.
//
// Types that implement Object can expose attributes that can be accessed
// from templates using dot notation: {{ obj.attribute_name }}.
//
// This is useful for exposing Go structs and custom types to templates
// with specific attribute access semantics.
//
// Example implementation:
//
//	type User struct {
//	    Username string
//	    Email    string
//	}
//
//	func (u *User) GetAttr(name string) Value {
//	    switch name {
//	    case "username":
//	        return FromString(u.Username)
//	    case "email":
//	        return FromString(u.Email)
//	    default:
//	        return Undefined()
//	    }
//	}
type Object interface {
	// GetAttr returns the value of the named attribute.
	//
	// If the attribute doesn't exist, returns Undefined().
	GetAttr(name string) Value
}

// MutableObject is an object that supports attribute assignment.
//
// Types that implement MutableObject can have their attributes set from
// templates, typically via {% set obj.attr = value %} syntax.
//
// This is primarily used for special objects like namespace() that need
// to support attribute assignment across scopes.
//
// Example implementation:
//
//	type Namespace struct {
//	    attrs map[string]Value
//	}
//
//	func (n *Namespace) GetAttr(name string) Value {
//	    if v, ok := n.attrs[name]; ok {
//	        return v
//	    }
//	    return Undefined()
//	}
//
//	func (n *Namespace) SetAttr(name string, val Value) {
//	    n.attrs[name] = val
//	}
type MutableObject interface {
	Object
	// SetAttr sets the value of the named attribute.
	SetAttr(name string, val Value)
}

// ValueKind describes the type of a Value.
//
// ValueKind is used to determine what type of data a Value contains without
// needing to perform type assertions. This allows for efficient type checking
// in templates and filters.
//
// Example usage:
//
//	val := FromString("hello")
//	if val.Kind() == KindString {
//	    s, _ := val.AsString()
//	    fmt.Println("String:", s)
//	}
type ValueKind int

const (
	// KindUndefined represents an undefined or missing value.
	//
	// Undefined values are returned when accessing non-existent variables,
	// attributes, or items. In templates, undefined values typically render
	// as empty strings unless strict undefined behavior is enabled.
	KindUndefined ValueKind = iota

	// KindNone represents a null/nil value.
	//
	// None is the equivalent of null in JSON, nil in Go, or None in Python.
	// It's used to explicitly represent the absence of a value.
	KindNone

	// KindBool represents a boolean value (true or false).
	//
	// Boolean values are used in conditionals and logic operations.
	KindBool

	// KindNumber represents a numeric value (integer or floating-point).
	//
	// Numbers can be either int64 or float64 internally, and support
	// arithmetic operations.
	KindNumber

	// KindString represents a text string.
	//
	// Strings can be either safe (pre-escaped) or unsafe (need escaping).
	// Safe strings are marked internally and won't be escaped again by
	// auto-escaping.
	KindString

	// KindBytes represents binary data.
	//
	// Bytes are similar to strings but represent raw binary data rather
	// than UTF-8 text.
	KindBytes

	// KindSeq represents an ordered sequence (array/list).
	//
	// Sequences can be iterated over and indexed by integer position.
	KindSeq

	// KindMap represents a key-value mapping (dict/object).
	//
	// Maps associate string keys with values and can be accessed using
	// dot notation or bracket notation in templates.
	KindMap

	// KindIterable represents a lazy iterator.
	//
	// Iterables are special sequences that are consumed when iterated.
	// They may not have a known length and can only be iterated once.
	KindIterable

	// KindCallable represents a callable object (function/macro).
	//
	// Callables can be invoked with arguments from templates.
	KindCallable

	// KindPlain represents a custom object with attribute access.
	//
	// Plain objects implement the Object interface and expose attributes
	// that can be accessed from templates.
	KindPlain

	// KindInvalid represents an invalid or corrupt value.
	//
	// This is rarely encountered and typically indicates an internal error.
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
	case KindCallable:
		return "callable"
	case KindPlain:
		return "plain object"
	case KindInvalid:
		return "invalid value"
	default:
		return "unknown"
	}
}

// Value represents a dynamically typed value in the template engine.
//
// Value is the core type used throughout MiniJinja for representing template
// data. It can hold any Go value and provides methods for type checking,
// conversion, and operations.
//
// Values are immutable for primitive types (strings, numbers, booleans) but
// sequences and maps are referenced, meaning modifications to the underlying
// Go slice or map will be visible through the Value.
//
// # Creating Values
//
// Values are typically created using constructor functions:
//
//	str := FromString("hello")
//	num := FromInt(42)
//	list := FromSlice([]Value{str, num})
//	dict := FromMap(map[string]Value{"key": str})
//
// Go values can also be automatically converted using FromAny:
//
//	val := FromAny(map[string]interface{}{
//	    "name": "Alice",
//	    "age": 30,
//	})
//
// # Type Checking
//
// Use the Kind() method to check the type:
//
//	if val.Kind() == KindString {
//	    // It's a string
//	}
//
// Or use type-specific checking methods:
//
//	if val.IsUndefined() { /* ... */ }
//	if val.IsTrue() { /* ... */ }
//
// # Type Conversion
//
// Use As* methods to convert to specific types:
//
//	if s, ok := val.AsString(); ok {
//	    fmt.Println("String value:", s)
//	}
//	if i, ok := val.AsInt(); ok {
//	    fmt.Println("Integer value:", i)
//	}
//
// # Operations
//
// Values support various operations defined in ops.go:
//
//	result, err := val1.Add(val2)  // arithmetic
//	cmp, ok := val1.Compare(val2)  // comparison
//	contains := val.Contains(item) // containment
type Value struct {
	data any
}

// internal marker types for special values
type undefinedType struct{}
type noneType struct{}

var (
	undefinedVal = undefinedType{}
	noneVal      = noneType{}
)

// Undefined returns the undefined value.
//
// Undefined represents a missing or non-existent value. In templates,
// undefined values typically render as empty strings unless strict
// undefined behavior is enabled.
//
// Example usage:
//
//	// Return undefined from a custom object
//	func (obj *MyObject) GetAttr(name string) Value {
//	    if name == "existing" {
//	        return FromString("value")
//	    }
//	    return Undefined()  // attribute doesn't exist
//	}
func Undefined() Value {
	return Value{data: undefinedVal}
}

// None returns the none/null value.
//
// None represents an explicit null value, equivalent to null in JSON,
// nil in Go, or None in Python.
//
// Example usage:
//
//	context := FromMap(map[string]Value{
//	    "name": FromString("Alice"),
//	    "age":  None(),  // explicitly null
//	})
func None() Value {
	return Value{data: noneVal}
}

// True returns the boolean true value.
//
// This is a convenience function equivalent to FromBool(true).
func True() Value {
	return Value{data: true}
}

// False returns the boolean false value.
//
// This is a convenience function equivalent to FromBool(false).
func False() Value {
	return Value{data: false}
}

// FromBool creates a Value from a boolean.
//
// Example usage:
//
//	isActive := FromBool(true)
//	// In template: {% if isActive %}...{% endif %}
func FromBool(v bool) Value {
	return Value{data: v}
}

// FromInt creates a Value from an int64.
//
// Integer values support arithmetic operations and can be compared.
//
// Example usage:
//
//	count := FromInt(42)
//	// In template: {{ count + 1 }}  -> 43
func FromInt(v int64) Value {
	return Value{data: v}
}

// FromFloat creates a Value from a float64.
//
// Floating-point values support arithmetic operations. Special values
// like infinity and NaN are handled according to Go's float64 semantics.
//
// Example usage:
//
//	price := FromFloat(19.99)
//	// In template: {{ price * 1.2 }}  -> 23.988
func FromFloat(v float64) Value {
	return Value{data: v}
}

// FromString creates a Value from a string.
//
// String values are not marked as safe and will be escaped by auto-escaping
// if enabled. For pre-escaped HTML, use FromSafeString instead.
//
// Example usage:
//
//	name := FromString("Alice")
//	// In template: {{ name }}  -> Alice
//	html := FromString("<b>Bold</b>")
//	// In template with escaping: {{ html }}  -> &lt;b&gt;Bold&lt;/b&gt;
func FromString(v string) Value {
	return Value{data: v}
}

// FromSafeString creates a Value from a safe (pre-escaped) string.
//
// Safe strings are marked as already escaped and will not be escaped again
// by auto-escaping. This is useful when you want to include HTML or other
// markup that should be rendered as-is.
//
// Example usage:
//
//	html := FromSafeString("<b>Bold</b>")
//	// In template with escaping: {{ html }}  -> <b>Bold</b>
//
// Use this with caution - ensure the string is actually safe to prevent XSS.
func FromSafeString(v string) Value {
	return Value{data: safeString(v)}
}

// safeString is an internal wrapper for strings that should not be escaped.
type safeString string

// bigIntValue wraps a big.Int for large integer values.
type bigIntValue struct {
	*big.Int
}

// Iterator represents a lazy iterator value.
// Unlike sequences, iterators are consumed when iterated and don't have a length.
type Iterator struct {
	items []Value
	name  string // e.g., "range", "reversed"
}

// NewIterator creates a new iterator from items.
func NewIterator(name string, items []Value) *Iterator {
	return &Iterator{items: items, name: name}
}

// FromIterator creates a Value from an Iterator.
func FromIterator(iter *Iterator) Value {
	return Value{data: iter}
}

// Items returns the iterator's items (consuming it conceptually).
func (i *Iterator) Items() []Value {
	return i.items
}

// FromBigInt creates a Value from a big.Int for arbitrary-precision integers.
//
// Big integers are used when values exceed the range of int64. They support
// the same arithmetic operations as regular integers.
//
// Example usage:
//
//	large := new(big.Int)
//	large.SetString("123456789012345678901234567890", 10)
//	val := FromBigInt(large)
//	// In template: {{ val + 1 }}
func FromBigInt(v *big.Int) Value {
	return Value{data: bigIntValue{v}}
}

// FromBytes creates a Value from a byte slice.
//
// Byte values represent binary data. They can be used for non-text content
// and are distinct from strings (which are UTF-8 text).
//
// Example usage:
//
//	data := FromBytes([]byte{0x48, 0x65, 0x6c, 0x6c, 0x6f})
//	// In template: {{ data }}  -> Hello
func FromBytes(v []byte) Value {
	return Value{data: v}
}

// FromSlice creates a Value from a slice of Values.
//
// Slices represent ordered sequences (arrays/lists) that can be iterated
// over and indexed by position. They correspond to arrays in JSON and
// lists in Python.
//
// Example usage:
//
//	items := FromSlice([]Value{
//	    FromString("apple"),
//	    FromString("banana"),
//	    FromString("cherry"),
//	})
//	// In template: {% for item in items %}{{ item }}{% endfor %}
//	// In template: {{ items[0] }}  -> apple
//	// In template: {{ items|length }}  -> 3
func FromSlice(v []Value) Value {
	return Value{data: v}
}

// FromMap creates a Value from a map of string to Value.
//
// Maps represent key-value associations (dictionaries/objects) that can be
// accessed using dot notation or bracket notation in templates. They
// correspond to objects in JSON and dicts in Python.
//
// Example usage:
//
//	user := FromMap(map[string]Value{
//	    "name":  FromString("Alice"),
//	    "age":   FromInt(30),
//	    "email": FromString("alice@example.com"),
//	})
//	// In template: {{ user.name }}  -> Alice
//	// In template: {{ user["age"] }}  -> 30
//	// In template: {% for key in user %}{{ key }}{% endfor %}
func FromMap(v map[string]Value) Value {
	return Value{data: v}
}

// FromCallable creates a Value from a Callable.
//
// Callables can be invoked from templates using function call syntax.
// This is useful for exposing custom functions to templates.
//
// Example usage:
//
//	type MyFunc struct{}
//	func (f *MyFunc) Call(args []Value, kwargs map[string]Value) (Value, error) {
//	    return FromString("Hello!"), nil
//	}
//
//	fn := FromCallable(&MyFunc{})
//	context := FromMap(map[string]Value{"greet": fn})
//	// In template: {{ greet() }}  -> Hello!
func FromCallable(c Callable) Value {
	return Value{data: c}
}

// FromObject creates a Value from an Object.
//
// Objects expose attributes that can be accessed from templates using dot
// notation. This is useful for exposing custom types with specific attribute
// access semantics.
//
// Example usage:
//
//	type User struct {
//	    Name string
//	    Age  int
//	}
//
//	func (u *User) GetAttr(name string) Value {
//	    switch name {
//	    case "name": return FromString(u.Name)
//	    case "age": return FromInt(int64(u.Age))
//	    default: return Undefined()
//	    }
//	}
//
//	user := &User{Name: "Alice", Age: 30}
//	val := FromObject(user)
//	// In template: {{ user.name }}  -> Alice
func FromObject(o Object) Value {
	return Value{data: o}
}

// FromAny creates a Value from any Go value using reflection.
//
// FromAny automatically converts Go types to their corresponding Value types:
//   - nil -> None()
//   - bool -> FromBool()
//   - int types -> FromInt()
//   - uint types -> FromInt()
//   - float types -> FromFloat()
//   - string -> FromString()
//   - []byte -> FromBytes()
//   - slices/arrays -> FromSlice() (recursively)
//   - maps -> FromMap() (recursively)
//   - structs -> FromMap() (using exported fields and json tags)
//   - pointers/interfaces -> dereference and convert
//
// This is the most convenient way to convert Go data to Values, but it
// uses reflection and may be slower than using specific constructors.
//
// Example usage:
//
//	data := FromAny(map[string]interface{}{
//	    "name": "Alice",
//	    "age":  30,
//	    "tags": []string{"admin", "user"},
//	})
//	// Equivalent to:
//	// FromMap(map[string]Value{
//	//     "name": FromString("Alice"),
//	//     "age": FromInt(30),
//	//     "tags": FromSlice([]Value{
//	//         FromString("admin"),
//	//         FromString("user"),
//	//     }),
//	// })
func FromAny(v any) Value {
	if v == nil {
		return None()
	}

	// Already a Value
	if val, ok := v.(Value); ok {
		return val
	}
	if obj, ok := v.(Object); ok {
		return FromObject(obj)
	}

	rv := reflect.ValueOf(v)
	return fromReflectValue(rv)
}

func fromReflectValue(rv reflect.Value) Value {
	if !rv.IsValid() {
		return None()
	}
	if rv.CanInterface() {
		if val, ok := rv.Interface().(Value); ok {
			return val
		}
	}

	switch rv.Kind() {
	case reflect.Bool:
		return FromBool(rv.Bool())
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return FromInt(rv.Int())
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		return FromInt(int64(rv.Uint()))
	case reflect.Float32, reflect.Float64:
		f := rv.Float()
		// Convert whole number floats to integers for consistency with JSON parsing
		if f == math.Trunc(f) && f >= math.MinInt64 && f <= math.MaxInt64 {
			return FromInt(int64(f))
		}
		return FromFloat(f)
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
	switch d := v.data.(type) {
	case undefinedType:
		return KindUndefined
	case noneType:
		return KindNone
	case bool:
		return KindBool
	case int64, float64, bigIntValue:
		return KindNumber
	case string, safeString:
		return KindString
	case []byte:
		return KindBytes
	case []Value:
		return KindSeq
	case map[string]Value:
		return KindMap
	case *Iterator:
		return KindIterable
	case Callable:
		return KindCallable
	case Object:
		// Check ObjectRepr to determine the appropriate kind
		switch GetObjectRepr(d) {
		case ObjectReprSeq:
			return KindSeq
		case ObjectReprMap:
			return KindMap
		case ObjectReprIterable:
			return KindIterable
		default:
			return KindPlain
		}
	default:
		return KindPlain
	}
}

// AsCallable returns the Callable if this value is callable.
// This returns true for both Callable and CallableObject implementations.
func (v Value) AsCallable() (Callable, bool) {
	if c, ok := v.data.(Callable); ok {
		return c, true
	}
	// Also check for CallableObject and wrap it
	if obj, ok := v.data.(Object); ok {
		if co, ok := obj.(CallableObject); ok {
			return &callableObjectWrapper{co}, true
		}
	}
	return nil, false
}

// callableObjectWrapper wraps a CallableObject as a Callable
type callableObjectWrapper struct {
	obj CallableObject
}

func (w *callableObjectWrapper) Call(state State, args []Value, kwargs map[string]Value) (Value, error) {
	return w.obj.ObjectCall(state, args, kwargs)
}

// IsCallable returns true if this value is callable.
func (v Value) IsCallable() bool {
	if _, ok := v.data.(Callable); ok {
		return true
	}
	if obj, ok := v.data.(Object); ok {
		if _, ok := obj.(CallableObject); ok {
			return true
		}
	}
	return false
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
	case Object:
		return GetObjectTruth(d)
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
	case bigIntValue:
		return d.String()
	case float64:
		// Match Jinja2's float formatting
		if math.IsInf(d, 1) {
			return "inf"
		}
		if math.IsInf(d, -1) {
			return "-inf"
		}
		if math.IsNaN(d) {
			return "nan"
		}
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
	case *Iterator:
		var parts []string
		for _, item := range d.items {
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
	case bigIntValue:
		return d.String()
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
	case *Iterator:
		return "<iterator>"
	case map[string]Value:
		var parts []string
		keys := make([]string, 0, len(d))
		for k := range d {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			parts = append(parts, fmt.Sprintf("%s: %s", formatMapKey(k), d[k].Repr()))
		}
		return "{" + strings.Join(parts, ", ") + "}"
	default:
		return fmt.Sprintf("%v", d)
	}
}

func formatMapKey(key string) string {
	if i, err := strconv.ParseInt(key, 10, 64); err == nil {
		if strconv.FormatInt(i, 10) == key {
			return key
		}
	}
	return fmt.Sprintf("%q", key)
}

// IsSafe returns true if this is a safe string.
func (v Value) IsSafe() bool {
	_, ok := v.data.(safeString)
	return ok
}

// IsActualInt returns true if the value is stored as an integer (not a float).
// This distinguishes 42 from 42.0.
func (v Value) IsActualInt() bool {
	_, ok := v.data.(int64)
	return ok
}

// IsActualFloat returns true if the value is stored as a float64.
func (v Value) IsActualFloat() bool {
	_, ok := v.data.(float64)
	return ok
}

// SameAs checks if two values are identical (stricter than equality).
// For objects and sequences/maps, checks if they are the same instance.
// For primitives, checks type match and value equality.
func (v Value) SameAs(other Value) bool {
	// Check if either is an object
	if obj1, ok1 := v.AsObject(); ok1 {
		if obj2, ok2 := other.AsObject(); ok2 {
			// Both are objects - check if same instance
			return obj1 == obj2
		}
		return false
	}
	if _, ok := other.AsObject(); ok {
		return false
	}

	// For sequences and maps, check pointer identity.
	// Two different slice/map literals are never "same as" each other.
	if v.Kind() == KindSeq || v.Kind() == KindMap {
		// In Go, slice identity is determined by the full slice header.
		// Compare pointer, length, and capacity to ensure reslices don't match.
		if s1, ok1 := v.data.([]Value); ok1 {
			if s2, ok2 := other.data.([]Value); ok2 {
				return reflect.ValueOf(s1).Pointer() == reflect.ValueOf(s2).Pointer() &&
					len(s1) == len(s2) &&
					cap(s1) == cap(s2)
			}
		}
		if m1, ok1 := v.data.(map[string]Value); ok1 {
			if m2, ok2 := other.data.(map[string]Value); ok2 {
				return reflect.ValueOf(m1).Pointer() == reflect.ValueOf(m2).Pointer()
			}
		}
		return false
	}

	// For primitives - check kind and int/float distinction
	if v.Kind() != other.Kind() {
		return false
	}
	if v.IsActualInt() != other.IsActualInt() {
		return false
	}
	return v.Equal(other)
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
	if m, ok := v.data.(MapGetter); ok {
		return m.Map(), true
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
	case *Iterator:
		return len(d.items), true
	case map[string]Value:
		return len(d), true
	case LenGetter:
		return d.Len()
	case Object:
		if length := GetObjectLen(d); length >= 0 {
			return length, true
		}
		return 0, false
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
	case *Iterator:
		if idx, ok := key.AsInt(); ok {
			if idx < 0 {
				idx = int64(len(d.items)) + idx
			}
			if idx >= 0 && idx < int64(len(d.items)) {
				return d.items[idx]
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
	case ItemGetter:
		return d.GetItem(key)
	case Object:
		// Check for SeqObject for index access
		if idx, ok := key.AsInt(); ok {
			if so, ok := d.(SeqObject); ok {
				length := so.SeqLen()
				if idx < 0 {
					idx = int64(length) + idx
				}
				if idx >= 0 && idx < int64(length) {
					return so.SeqItem(int(idx))
				}
				return Undefined()
			}
		}
		// Fall through to attribute access for string keys
		if s, ok := key.AsString(); ok {
			return d.GetAttr(s)
		}
		return Undefined()
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
	case Object:
		return d.GetAttr(name)
	}
	return Undefined()
}

// AsObject returns the Object if this value wraps one.
func (v Value) AsObject() (Object, bool) {
	if o, ok := v.data.(Object); ok {
		return o, true
	}
	return nil, false
}

// AsMutableObject returns the MutableObject if this value wraps one.
func (v Value) AsMutableObject() (MutableObject, bool) {
	if o, ok := v.data.(MutableObject); ok {
		return o, true
	}
	return nil, false
}

// Iterable is an interface for objects that can be iterated.
type Iterable interface {
	Iter() []Value
}

// LenGetter is an interface for objects that have a length.
type LenGetter interface {
	Len() (int, bool)
}

// ItemGetter is an interface for objects that support item access.
type ItemGetter interface {
	GetItem(key Value) Value
}

// MapGetter is an interface for objects that expose mapping data.
type MapGetter interface {
	Map() map[string]Value
}

// Iter returns an iterator over the value's items.
func (v Value) Iter() []Value {
	switch d := v.data.(type) {
	case []Value:
		return d
	case *Iterator:
		return d.items
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
	case Iterable:
		return d.Iter()
	case Object:
		// Check for new object iteration interfaces
		if seq := IterateObject(d); seq != nil {
			var result []Value
			for item := range seq {
				result = append(result, item)
			}
			return result
		}
		return nil
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
