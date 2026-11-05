package filters

import (
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/fuel"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// Fuel helpers for the built-in filters.
//
// The tracker of a state is found through an internal lookup, so states
// that are not a *minijinja.State (or that do not track fuel) are not
// charged.  All helpers are no-ops in that case.

func fuelOf(state State) *fuel.Tracker {
	if state == nil {
		return nil
	}
	return fuel.Of(state)
}

// fuelItems returns the items a filter iterates over. Like Value::try_iter in
// Rust, none and undefined are treated as empty sequences, so `none|sort` gives
// `[]` and `none|join` gives an empty string. It returns nil for other values
// that cannot be iterated.
//
// Every item is charged and host objects stop iterating once the fuel runs
// out.
func fuelItems(state State, val value.Value) ([]value.Value, error) {
	if val.IsNone() || val.IsUndefined() {
		return []value.Value{}, nil
	}
	return fuelOf(state).Items(val)
}

// tryFuelItems is like fuelItems, but like UndefinedBehavior::try_iter in
// Rust it fails for undefined values with strict or semi-strict undefined
// behavior.
func tryFuelItems(state State, val value.Value) ([]value.Value, error) {
	if err := assertNotUndefined(state, val); err != nil {
		return nil, err
	}
	return fuelItems(state, val)
}

// chargeItems charges for visiting n items.
func chargeItems(state State, n int) error {
	return fuelOf(state).Consume(fuel.CostOf(n, fuel.ItemCost))
}

// chargeCall charges for a call to a filter or test made on behalf of a
// built-in filter.  This is charged like the call from a template: one
// instruction plus the string the call works on.
func chargeCall(t *fuel.Tracker, input value.Value) error {
	if t == nil {
		return nil
	}
	cost := fuel.InstructionCost
	if n, ok := fuel.StringLen(input); ok {
		cost += fuel.CostOf(n, fuel.ByteCost)
	}
	return t.Consume(cost)
}

// chargeValue charges for the size of a value.
func chargeValue(t *fuel.Tracker, val value.Value) error {
	if t == nil {
		return nil
	}
	return t.Consume(fuel.ValueCost(val))
}

// chargeSort charges for sorting n items.
func chargeSort(state State, n int) error {
	return fuelOf(state).Consume(fuel.SortCost(n))
}

// chargeLookup charges for looking up an item among n sorted items.
func chargeLookup(state State, n int) error {
	return fuelOf(state).Consume(fuel.LookupCost(n))
}

// chargeSerialized charges the extra cost of serializing a value into n
// bytes.  The engine already charges for the bytes of the resulting string.
func chargeSerialized(state State, n int) error {
	return fuelOf(state).Consume(fuel.CostOf(n, fuel.SerializedByteCost-fuel.ByteCost))
}

// chargeFormatSpec charges for one format specifier of the format filter.
func chargeFormatSpec(state State) error {
	return fuelOf(state).Consume(fuel.FormatSpecCost)
}

// chargeBytes charges for n bytes.
func chargeBytes(state State, n int) error {
	return fuelOf(state).Consume(fuel.CostOf(n, fuel.ByteCost))
}

// ensureBytes checks that n bytes could still be paid for.
func ensureBytes(state State, n int) error {
	return fuelOf(state).Ensure(fuel.CostOf(n, fuel.ByteCost))
}

// ensureBytesQuiet is like ensureBytes but does not exhaust the fuel if the
// check fails.
func ensureBytesQuiet(state State, n int) error {
	return fuelOf(state).EnsureQuiet(fuel.CostOf(n, fuel.ByteCost))
}

// ensureSlots checks that n list slots could still be paid for.
func ensureSlots(state State, n int) error {
	return fuelOf(state).Ensure(fuel.CostOf(n, fuel.SlotCost))
}

// growBuilder makes room for n more bytes in a builder.  Large builders only
// grow by a quarter when appending, which makes the total allocation many
// times the result.  Doubling keeps it at about twice the result.
func growBuilder(b *strings.Builder, n int) {
	if b.Cap()-b.Len() < n {
		b.Grow(max(n, b.Len()))
	}
}

// saturatingAdd adds two non-negative ints without overflowing.
func saturatingAdd(a, b int) int {
	if a > maxInt-b {
		return maxInt
	}
	return a + b
}

// saturatingMul multiplies two non-negative ints without overflowing.
func saturatingMul(a, b int) int {
	if a != 0 && b > maxInt/a {
		return maxInt
	}
	return a * b
}

const maxInt = int(^uint(0) >> 1)
