package value

import (
	"context"
	"errors"
	"fmt"
	"iter"
	"sync"
)

// ErrUnknownMethod is returned by MethodCallable.CallMethod when the method
// is not known. This signals the engine to fall back to GetAttr + call.
var ErrUnknownMethod = errors.New("unknown method")

// State is the interface that the template engine's state must implement.
// This allows objects to access template state without circular dependencies.
type State interface {
	// Context returns the Go context for this render operation.
	Context() context.Context

	// Lookup looks up a variable by name in the current scope.
	Lookup(name string) Value

	// Name returns the name of the template being rendered.
	Name() string
}

// ObjectRepr indicates the natural representation of an object.
type ObjectRepr int

const (
	// ObjectReprPlain is a plain object with no special iteration behavior.
	// This is the default for objects. They render as debug output and
	// are not iterable.
	ObjectReprPlain ObjectRepr = iota

	// ObjectReprMap indicates the object behaves like a map/dict.
	// Iteration yields keys, and values are accessed via GetAttr.
	ObjectReprMap

	// ObjectReprSeq indicates the object behaves like a sequence/array.
	// Iteration yields values from index 0 to SeqLen()-1.
	ObjectReprSeq

	// ObjectReprIterable indicates the object is a generic iterable.
	// It can be iterated but may not support indexing or have known length.
	ObjectReprIterable
)

func (r ObjectRepr) String() string {
	switch r {
	case ObjectReprPlain:
		return "plain"
	case ObjectReprMap:
		return "map"
	case ObjectReprSeq:
		return "seq"
	case ObjectReprIterable:
		return "iterable"
	default:
		return "unknown"
	}
}

// -----------------------------------------------------------------------------
// Optional Object Interfaces
// -----------------------------------------------------------------------------

// ObjectWithRepr allows objects to specify their representation type.
// If not implemented, defaults to ObjectReprPlain.
type ObjectWithRepr interface {
	Object
	// ObjectRepr returns the natural representation of this object.
	ObjectRepr() ObjectRepr
}

// SeqObject is an object that behaves like a sequence.
// Implement this along with ObjectWithRepr returning ObjectReprSeq.
type SeqObject interface {
	Object
	// SeqLen returns the length of the sequence.
	SeqLen() int
	// SeqItem returns the item at index (0-based).
	// Returns Undefined() if index is out of bounds.
	SeqItem(index int) Value
}

// MapObject is an object that behaves like a map with known keys.
// The engine iterates over Keys() and calls GetAttr for values.
type MapObject interface {
	Object
	// Keys returns the keys that this map contains.
	// Used for iteration, length calculation, and debug output.
	Keys() []string
}

// IterableObject can provide a custom iterator.
// This takes precedence over SeqObject/MapObject for iteration.
type IterableObject interface {
	Object
	// Iterate returns an iterator over the object's values.
	// Called each time iteration is needed (allows multiple iterations).
	// Return nil to indicate the object is not iterable.
	Iterate() iter.Seq[Value]
}

// CallableObject is an object that can be called like a function.
// This is for objects that are themselves callable (not their methods).
//
// Example: cycler = make_cycler("a", "b"); cycler() returns next value
type CallableObject interface {
	Object
	// ObjectCall invokes the object itself with the given arguments.
	ObjectCall(state State, args []Value, kwargs map[string]Value) (Value, error)
}

// MethodCallable is an object that supports method calls.
// This allows `obj.method(args)` syntax to invoke methods on the object.
//
// Example: magic.make_class("ul") calls the make_class method on magic
type MethodCallable interface {
	Object
	// CallMethod invokes a method on the object.
	// Return ErrUnknownMethod to fall back to GetAttr(name) + call.
	CallMethod(state State, name string, args []Value, kwargs map[string]Value) (Value, error)
}

// ObjectWithLen provides explicit length for an object.
// If not implemented, length is derived from SeqObject or MapObject.
type ObjectWithLen interface {
	Object
	// ObjectLen returns the length of the object.
	// Return -1 if length is unknown.
	ObjectLen() int
}

// ObjectWithTruth provides custom truthiness for an object.
// If not implemented, truthiness is based on length (empty = false).
type ObjectWithTruth interface {
	Object
	// ObjectIsTrue returns whether this object is considered "true".
	ObjectIsTrue() bool
}

// ObjectWithString provides custom string representation.
// If not implemented, uses default formatting based on ObjectRepr.
type ObjectWithString interface {
	Object
	// ObjectString returns the string representation of this object.
	ObjectString() string
}

// ObjectWithCmp provides custom comparison for sorting.
// If not implemented, objects are compared by identity only.
//
// Example:
//
//	type Thing struct { num int }
//
//	func (t *Thing) ObjectCmp(other Object) (int, bool) {
//	    if ot, ok := other.(*Thing); ok {
//	        return t.num - ot.num, true
//	    }
//	    return 0, false  // incomparable types
//	}
type ObjectWithCmp interface {
	Object
	// ObjectCmp compares this object with another.
	// Returns (cmp, ok) where:
	//   - cmp < 0 means this < other
	//   - cmp == 0 means this == other
	//   - cmp > 0 means this > other
	//   - ok == false means the objects are incomparable
	ObjectCmp(other Object) (cmp int, ok bool)
}

// ReversibleObject can provide efficient reverse iteration.
// If not implemented, reverse() will collect and reverse the items.
//
// This is useful for sequences that can be efficiently iterated in reverse
// without collecting all items first.
type ReversibleObject interface {
	Object
	// ReverseIterate returns an iterator that yields items in reverse order.
	// Return nil to fall back to collect-and-reverse behavior.
	ReverseIterate() iter.Seq[Value]
}

// PullIterator is an interface for pull-based iteration.
// This is used for one-shot iterators that need to preserve items across
// partial iterations (e.g., when a loop uses break).
//
// Unlike regular iterables that are collected into slices, PullIterators
// allow item-by-item consumption with the ability to stop and resume.
type PullIterator interface {
	// PullNext returns the next item and whether there are more items.
	// Returns (value, true) for the next item, or (undefined, false) when exhausted.
	PullNext() (Value, bool)

	// PullDone returns true if the iterator is exhausted.
	PullDone() bool
}

// -----------------------------------------------------------------------------
// Helper Functions for Objects
// -----------------------------------------------------------------------------

// GetObjectRepr returns the representation type of an object.
func GetObjectRepr(obj Object) ObjectRepr {
	if r, ok := obj.(ObjectWithRepr); ok {
		return r.ObjectRepr()
	}
	return ObjectReprPlain
}

// GetObjectLen returns the length of an object, or -1 if unknown.
func GetObjectLen(obj Object) int {
	if l, ok := obj.(ObjectWithLen); ok {
		return l.ObjectLen()
	}
	if s, ok := obj.(SeqObject); ok {
		return s.SeqLen()
	}
	if m, ok := obj.(MapObject); ok {
		return len(m.Keys())
	}
	return -1
}

// GetObjectTruth returns the truthiness of an object.
func GetObjectTruth(obj Object) bool {
	if t, ok := obj.(ObjectWithTruth); ok {
		return t.ObjectIsTrue()
	}
	length := GetObjectLen(obj)
	if length >= 0 {
		return length > 0
	}
	// Unknown length - assume true (non-empty)
	return true
}

// IterateObject returns an iterator over an object's values.
// Returns nil if the object is not iterable.
func IterateObject(obj Object) iter.Seq[Value] {
	// Check for explicit iterable first
	if it, ok := obj.(IterableObject); ok {
		if seq := it.Iterate(); seq != nil {
			return seq
		}
	}

	repr := GetObjectRepr(obj)

	// Sequence iteration
	if repr == ObjectReprSeq {
		if s, ok := obj.(SeqObject); ok {
			return func(yield func(Value) bool) {
				for i := 0; i < s.SeqLen(); i++ {
					if !yield(s.SeqItem(i)) {
						return
					}
				}
			}
		}
	}

	// Map iteration (yields keys)
	if repr == ObjectReprMap || repr == ObjectReprPlain {
		if m, ok := obj.(MapObject); ok {
			return func(yield func(Value) bool) {
				for _, k := range m.Keys() {
					if !yield(FromString(k)) {
						return
					}
				}
			}
		}
	}

	return nil
}

// ReverseIterateObject returns a reverse iterator over an object's values.
// Returns nil if the object cannot be reverse-iterated.
//
// Priority:
//  1. ReversibleObject.ReverseIterate() if implemented
//  2. SeqObject iterated in reverse (index n-1 to 0)
//  3. nil (caller should collect and reverse)
func ReverseIterateObject(obj Object) iter.Seq[Value] {
	// Check for explicit reverse iterator first
	if rev, ok := obj.(ReversibleObject); ok {
		if seq := rev.ReverseIterate(); seq != nil {
			return seq
		}
	}

	// Sequence can be efficiently reversed
	repr := GetObjectRepr(obj)
	if repr == ObjectReprSeq {
		if s, ok := obj.(SeqObject); ok {
			return func(yield func(Value) bool) {
				for i := s.SeqLen() - 1; i >= 0; i-- {
					if !yield(s.SeqItem(i)) {
						return
					}
				}
			}
		}
	}

	// Maps can be reversed by reversing keys
	if repr == ObjectReprMap || repr == ObjectReprPlain {
		if m, ok := obj.(MapObject); ok {
			return func(yield func(Value) bool) {
				keys := m.Keys()
				for i := len(keys) - 1; i >= 0; i-- {
					if !yield(FromString(keys[i])) {
						return
					}
				}
			}
		}
	}

	return nil
}

// CompareObjects compares two objects using ObjectWithCmp if available.
// Returns (cmp, ok) where ok is false if objects are incomparable.
func CompareObjects(a, b Object) (int, bool) {
	if cmp, ok := a.(ObjectWithCmp); ok {
		return cmp.ObjectCmp(b)
	}
	return 0, false
}

// -----------------------------------------------------------------------------
// MakeIterable - Create lazy iterable values
// -----------------------------------------------------------------------------

// iterableObject wraps a maker function as an iterable object.
type iterableObject struct {
	maker func() iter.Seq[Value]
}

func (i *iterableObject) GetAttr(name string) Value {
	return Undefined()
}

func (i *iterableObject) ObjectRepr() ObjectRepr {
	return ObjectReprIterable
}

func (i *iterableObject) Iterate() iter.Seq[Value] {
	return i.maker()
}

func (i *iterableObject) String() string {
	return "<iterator>"
}

// MakeIterable creates a lazy iterable value from a maker function.
// The maker is called each time iteration is needed, allowing multiple iterations.
//
// Example:
//
//	val := MakeIterable(func() iter.Seq[Value] {
//	    return func(yield func(Value) bool) {
//	        for i := 0; i < 10; i++ {
//	            if !yield(FromInt(int64(i))) {
//	                return
//	            }
//	        }
//	    }
//	})
func MakeIterable(maker func() iter.Seq[Value]) Value {
	return FromObject(&iterableObject{maker: maker})
}

// -----------------------------------------------------------------------------
// MakeOneShotIterator - Create one-shot iterators
// -----------------------------------------------------------------------------

// oneShotIterator is an iterator that can only be consumed once.
// After the first complete iteration, subsequent iterations yield nothing.
type oneShotIterator struct {
	mu   sync.Mutex
	next func() (Value, bool) // Pull-based iterator
	stop func()               // Cleanup function
	done bool                 // Whether the underlying iterator is exhausted
}

func (o *oneShotIterator) GetAttr(name string) Value {
	return Undefined()
}

func (o *oneShotIterator) ObjectRepr() ObjectRepr {
	return ObjectReprIterable
}

// ObjectLen returns -1 to indicate unknown length.
// This is important for loop.length to show "?" instead of a number.
func (o *oneShotIterator) ObjectLen() int {
	return -1
}

// PullNext returns the next item (implements PullIterator).
func (o *oneShotIterator) PullNext() (Value, bool) {
	o.mu.Lock()
	defer o.mu.Unlock()

	if o.done || o.next == nil {
		return Undefined(), false
	}

	v, ok := o.next()
	if !ok {
		o.done = true
		if o.stop != nil {
			o.stop()
			o.stop = nil
		}
		return Undefined(), false
	}
	return v, true
}

// PullDone returns true if the iterator is exhausted (implements PullIterator).
func (o *oneShotIterator) PullDone() bool {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.done
}

func (o *oneShotIterator) Iterate() iter.Seq[Value] {
	return func(yield func(Value) bool) {
		// Pull items one at a time
		for {
			v, ok := o.PullNext()
			if !ok {
				return
			}
			if !yield(v) {
				// Consumer stopped early - remaining items stay in the iterator
				return
			}
		}
	}
}

func (o *oneShotIterator) String() string {
	return "<iterator>"
}

// MakeOneShotIterator creates an iterator that can only be consumed once.
//
// Unlike MakeIterable, a one-shot iterator:
//   - Can only be fully iterated once
//   - Has no known length (loop.length is undefined)
//   - Renders as "<iterator>"
//   - After consumption, subsequent iterations yield nothing
//
// If iteration is stopped early (e.g., with break), the remaining items
// are preserved for the next iteration attempt.
//
// Example:
//
//	val := MakeOneShotIterator(func(yield func(Value) bool) {
//	    for i := 0; i < 10; i++ {
//	        if !yield(FromInt(int64(i))) {
//	            return
//	        }
//	    }
//	})
//
//	// First iteration: yields 0, 1, 2, ..., 9
//	// Second iteration: yields nothing
func MakeOneShotIterator(seq iter.Seq[Value]) Value {
	// Convert push-based iterator to pull-based using iter.Pull
	next, stop := iter.Pull(seq)
	return FromObject(&oneShotIterator{
		next: next,
		stop: stop,
	})
}

// MakeIterableFromSlice creates an iterable that yields values from a slice.
// The slice function is called each time to get fresh values.
func MakeIterableFromSlice(maker func() []Value) Value {
	return MakeIterable(func() iter.Seq[Value] {
		items := maker()
		return func(yield func(Value) bool) {
			for _, item := range items {
				if !yield(item) {
					return
				}
			}
		}
	})
}

// -----------------------------------------------------------------------------
// MakeObjectMap - Create map projections
// -----------------------------------------------------------------------------

// objectMapProjection projects callbacks as a map-like object.
type objectMapProjection struct {
	enumerate func() iter.Seq[Value]
	getAttr   func(key Value) Value
	// Cache for keys (for length calculation)
	keysOnce sync.Once
	keys     []Value
}

func (m *objectMapProjection) GetAttr(name string) Value {
	return m.getAttr(FromString(name))
}

func (m *objectMapProjection) ObjectRepr() ObjectRepr {
	return ObjectReprMap
}

func (m *objectMapProjection) Iterate() iter.Seq[Value] {
	return m.enumerate()
}

func (m *objectMapProjection) getKeys() []Value {
	m.keysOnce.Do(func() {
		for v := range m.enumerate() {
			m.keys = append(m.keys, v)
		}
	})
	return m.keys
}

func (m *objectMapProjection) ObjectLen() int {
	return len(m.getKeys())
}

func (m *objectMapProjection) Keys() []string {
	keys := m.getKeys()
	result := make([]string, 0, len(keys))
	for _, k := range keys {
		if s, ok := k.AsString(); ok {
			result = append(result, s)
		} else {
			result = append(result, k.String())
		}
	}
	return result
}

func (m *objectMapProjection) String() string {
	keys := m.getKeys()
	parts := make([]string, 0, len(keys))
	for _, k := range keys {
		v := m.getAttr(k)
		parts = append(parts, fmt.Sprintf("%s: %s", k.Repr(), v.Repr()))
	}
	return "{" + fmt.Sprintf("%s", joinStrings(parts, ", ")) + "}"
}

func joinStrings(parts []string, sep string) string {
	if len(parts) == 0 {
		return ""
	}
	result := parts[0]
	for _, p := range parts[1:] {
		result += sep + p
	}
	return result
}

// MakeObjectMap creates a map-like value that projects onto callbacks.
//
// Parameters:
//   - enumerate: returns an iterator over the map's keys
//   - getAttr: returns the value for a given key
//
// Example (projecting a Go map as a Value):
//
//	attrs := map[string]string{"id": "link-1", "class": "links"}
//	val := MakeObjectMap(
//	    func() iter.Seq[Value] {
//	        return func(yield func(Value) bool) {
//	            for k := range attrs {
//	                if !yield(FromString(k)) {
//	                    return
//	                }
//	            }
//	        }
//	    },
//	    func(key Value) Value {
//	        if s, ok := key.AsString(); ok {
//	            if v, exists := attrs[s]; exists {
//	                return FromString(v)
//	            }
//	        }
//	        return Undefined()
//	    },
//	)
func MakeObjectMap(enumerate func() iter.Seq[Value], getAttr func(key Value) Value) Value {
	return FromObject(&objectMapProjection{
		enumerate: enumerate,
		getAttr:   getAttr,
	})
}

// GetItem for objectMapProjection - allow both string and index access
func (m *objectMapProjection) GetItem(key Value) Value {
	return m.getAttr(key)
}
