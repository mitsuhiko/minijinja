// Package fuel implements fuel tracking.
//
// Fuel is a single budget for all the work a template can make the engine
// do.  Every evaluated statement and expression costs one fuel, and on top of
// that the engine charges for the data it produces and the items it visits:
//
//   - every byte written to the output (including captured output such as
//     {% set %} blocks and macro bodies),
//   - every byte of strings and every slot of lists and maps created by
//     operators (~, +, *, slicing), literals and the results of filters,
//     functions and methods,
//   - every byte of string input to a filter or method,
//   - every item that a built-in filter or operator visits, plus the cost of
//     comparisons for sorting.
//
// Internally fuel is tracked in units of [UnitsPerFuel] so that cheap
// operations such as individual bytes can be charged without rounding.
//
// Allocations whose size is known ahead of time are checked before they are
// performed (see [Tracker.Ensure]) so that a single huge allocation cannot
// slip through.
//
// Once the fuel runs out, the tracker stays exhausted: every later charge
// fails.  That way code that cannot propagate an error directly can stop
// early and the engine fails at the next check.
package fuel

import (
	"math"
	"math/bits"
	"slices"
	"sync/atomic"
	"unicode/utf8"

	mjerrors "github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/errors"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

const (
	// UnitsPerFuel is the internal resolution of fuel.  One fuel is worth
	// this many units, and one byte costs one unit.
	UnitsPerFuel uint64 = 64

	// InstructionCost is the cost of one evaluated statement or expression.
	InstructionCost uint64 = UnitsPerFuel

	// ByteCost is the cost of one byte of data produced or processed.
	ByteCost uint64 = 1

	// ItemCost is the cost of visiting one item in a built-in filter or
	// operator.
	ItemCost uint64 = 32

	// CompareCost is the cost of comparing two values while sorting or
	// deduplicating.
	CompareCost uint64 = 64

	// SlotCost is the cost of one slot in a list (half that of a map entry).
	SlotCost uint64 = 32

	// SerializedByteCost is the cost of one byte of serialized output
	// (tojson, pprint and converting containers to strings).
	SerializedByteCost uint64 = 16

	// FormatSpecCost is the cost of one format specifier in the format
	// filter.
	FormatSpecCost uint64 = 4 * UnitsPerFuel
)

// maxCharge caps a single charge so that subtracting it can never overflow.
const maxCharge = uint64(1) << 62

// Tracker tracks the fuel of a render.
//
// All methods are safe to call on a nil tracker, in which case they do
// nothing.
type Tracker struct {
	initial   uint64
	remaining atomic.Int64 // in units
}

// New creates a new tracker with the given fuel.
func New(fuel uint64) *Tracker {
	t := &Tracker{initial: fuel}
	units := uint64(math.MaxInt64)
	if fuel <= math.MaxInt64/UnitsPerFuel {
		units = fuel * UnitsPerFuel
	}
	t.remaining.Store(int64(units))
	return t
}

func outOfFuel() error {
	return mjerrors.NewError(mjerrors.ErrOutOfFuel, "out of fuel")
}

// Consume consumes the given number of units.
//
// It fails if this exhausts the fuel or if the fuel was already exhausted.
func (t *Tracker) Consume(units uint64) error {
	if t == nil {
		return nil
	}
	if units > maxCharge {
		units = maxCharge
	}
	if t.remaining.Add(-int64(units)) <= 0 {
		// keep the counter from drifting towards an overflow
		t.remaining.Store(0)
		return outOfFuel()
	}
	return nil
}

// Ensure checks that the given number of units could still be consumed.
//
// This is used before allocating something of a known size.  It does not
// consume the fuel as the result is charged once it exists, but if the fuel
// would run out, the tracker is exhausted immediately.
func (t *Tracker) Ensure(units uint64) error {
	if t == nil {
		return nil
	}
	remaining := t.remaining.Load()
	if remaining <= 0 || units >= uint64(remaining) {
		t.remaining.Store(0)
		return outOfFuel()
	}
	return nil
}

// EnsureQuiet is like [Tracker.Ensure] but does not exhaust the fuel if the
// check fails.
func (t *Tracker) EnsureQuiet(units uint64) error {
	if t == nil {
		return nil
	}
	remaining := t.remaining.Load()
	if remaining <= 0 || units >= uint64(remaining) {
		return outOfFuel()
	}
	return nil
}

// RemainingUnits returns the remaining fuel in units.
func (t *Tracker) RemainingUnits() uint64 {
	if t == nil {
		return 0
	}
	remaining := t.remaining.Load()
	if remaining <= 0 {
		return 0
	}
	return uint64(remaining)
}

// Remaining returns the remaining fuel.
func (t *Tracker) Remaining() uint64 {
	return t.RemainingUnits() / UnitsPerFuel
}

// Consumed returns the consumed fuel.
func (t *Tracker) Consumed() uint64 {
	if t == nil {
		return 0
	}
	remaining := t.Remaining()
	if remaining >= t.initial {
		return 0
	}
	return t.initial - remaining
}

// CostOf returns the cost of n things of the given unit cost.
func CostOf(n int, unit uint64) uint64 {
	if n <= 0 {
		return 0
	}
	hi, lo := bits.Mul64(uint64(n), unit)
	if hi != 0 {
		return math.MaxUint64
	}
	return lo
}

// SortCost returns the cost of sorting n items.
func SortCost(n int) uint64 {
	return CostOf(n, LookupCost(n))
}

// LookupCost returns the cost of looking up an item among n sorted items.
func LookupCost(n int) uint64 {
	if n <= 0 {
		return 0
	}
	return CompareCost * uint64(bits.Len64(uint64(n)))
}

// ValueCost returns the size cost of a value.
//
// This is the cost of the value itself, not of the values it contains: a
// string costs its length and a list costs a slot per item.  Items were
// already charged when they were created.  Objects are free as the engine
// cannot know how large they are.
//
// Unlike in the Rust implementation, iterators (such as the result of
// range() or reverse) are materialized in Go, so they cost a slot per item
// too.
func ValueCost(v value.Value) uint64 {
	switch d := v.Raw().(type) {
	case string:
		return CostOf(len(d), ByteCost)
	case []byte:
		return CostOf(len(d), ByteCost)
	case []value.Value:
		return CostOf(len(d), SlotCost)
	case map[string]value.Value:
		return CostOf(len(d), 2*SlotCost)
	case *value.Map:
		return CostOf(d.Len(), 2*SlotCost)
	case *value.Iterator:
		return CostOf(len(d.Items()), SlotCost)
	}
	switch v.Kind() {
	case value.KindString:
		// safe strings
		s, _ := v.AsString()
		return CostOf(len(s), ByteCost)
	case value.KindSeq:
		if v.IsTuple() {
			items, _ := v.AsSlice()
			return CostOf(len(items), SlotCost)
		}
	}
	return 0
}

// StringLen returns the byte length of a string value.
func StringLen(v value.Value) (int, bool) {
	if v.Kind() != value.KindString {
		return 0, false
	}
	s, _ := v.AsString()
	return len(s), true
}

// IsMaterialized returns true if iterating the value does not have to pull
// items from a host object.
func IsMaterialized(v value.Value) bool {
	switch v.Raw().(type) {
	case []value.Value, map[string]value.Value, *value.Map, *value.Iterator, string, []byte:
		return true
	case value.Iterable:
		return true
	case value.Object:
		return false
	}
	return true
}

// Items returns the items of a value like [value.Value.Iter] and charges
// [ItemCost] for every item.
//
// Values that are not materialized (host objects) are iterated lazily and the
// iteration stops as soon as the fuel runs out, so that infinite iterables
// fail with an out of fuel error.  For materialized values with a known size
// the cost is checked before the items are created.
//
// Like [value.Value.Iter] it returns nil (and no error) if the value cannot
// be iterated.  Without a tracker this is just [value.Value.Iter].
func (t *Tracker) Items(v value.Value) ([]value.Value, error) {
	if t == nil {
		return v.Iter(), nil
	}
	switch d := v.Raw().(type) {
	case []value.Value, *value.Iterator:
		items := v.Iter()
		return items, t.Consume(CostOf(len(items), ItemCost))
	case map[string]value.Value:
		if err := t.Ensure(CostOf(len(d), ItemCost)); err != nil {
			return nil, err
		}
		items := v.Iter()
		return items, t.Consume(CostOf(len(items), ItemCost))
	case *value.Map:
		if err := t.Ensure(CostOf(d.Len(), ItemCost)); err != nil {
			return nil, err
		}
		items := v.Iter()
		return items, t.Consume(CostOf(len(items), ItemCost))
	case string:
		if err := t.Ensure(CostOf(utf8.RuneCountInString(d), ItemCost)); err != nil {
			return nil, err
		}
		items := v.Iter()
		return items, t.Consume(CostOf(len(items), ItemCost))
	case value.Iterable:
		items := d.Iter()
		if items == nil {
			items = []value.Value{}
		}
		return items, t.Consume(CostOf(len(items), ItemCost))
	case value.Object:
		seq := value.IterateObject(d)
		if seq == nil {
			return nil, nil
		}
		items := []value.Value{}
		var err error
		for item := range seq {
			if err = t.Consume(ItemCost); err != nil {
				break
			}
			// grow by doubling so that the total allocation stays at
			// about twice the final size.
			if len(items) == cap(items) {
				items = slices.Grow(items, max(len(items), 16))
			}
			items = append(items, item)
		}
		if err != nil {
			return nil, err
		}
		return items, nil
	}
	if v.Kind() == value.KindString {
		s, _ := v.AsString()
		if err := t.Ensure(CostOf(utf8.RuneCountInString(s), ItemCost)); err != nil {
			return nil, err
		}
	}
	items := v.Iter()
	return items, t.Consume(CostOf(len(items), ItemCost))
}

// Contains checks if a container contains a value and charges for the work.
//
// Strings cost their byte length, sequences and iterables [ItemCost] per item
// visited.  Host iterables are iterated lazily.
func (t *Tracker) Contains(container, item value.Value) (bool, error) {
	if t == nil {
		return container.Contains(item), nil
	}
	if s, ok := StringLen(container); ok {
		if err := t.Consume(CostOf(s, ByteCost)); err != nil {
			return false, err
		}
		return container.Contains(item), nil
	}
	switch container.Kind() {
	case value.KindSeq, value.KindIterable:
	default:
		return container.Contains(item), nil
	}
	if !IsMaterialized(container) {
		obj, _ := container.AsObject()
		seq := value.IterateObject(obj)
		if seq == nil {
			return false, nil
		}
		for x := range seq {
			if err := t.Consume(ItemCost); err != nil {
				return false, err
			}
			if x.Equal(item) {
				return true, nil
			}
		}
		return false, nil
	}
	items := container.Iter()
	visited := len(items)
	found := false
	for i, x := range items {
		if x.Equal(item) {
			visited = i + 1
			found = true
			break
		}
	}
	return found, t.Consume(CostOf(visited, ItemCost))
}

var stateLookup atomic.Pointer[func(any) *Tracker]

// SetStateLookup registers the function that finds the tracker of a state.
//
// The minijinja package registers this so that the built-in filters and
// tests can charge fuel without the state exposing the tracker publicly.
func SetStateLookup(lookup func(any) *Tracker) {
	stateLookup.Store(&lookup)
}

// Of returns the tracker of a state or nil if the state does not track fuel.
func Of(state any) *Tracker {
	if state == nil {
		return nil
	}
	if lookup := stateLookup.Load(); lookup != nil {
		return (*lookup)(state)
	}
	return nil
}
