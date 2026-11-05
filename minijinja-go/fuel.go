package minijinja

import (
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/fuel"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

type fuelTracker = fuel.Tracker

func newFuelTracker(amount uint64) *fuelTracker {
	return fuel.New(amount)
}

func init() {
	// the built-in filters and tests find the tracker of a state through
	// this lookup so that the tracker does not need to be public.
	fuel.SetStateLookup(func(state any) *fuel.Tracker {
		if s, ok := state.(*State); ok {
			return s.fuelTracker
		}
		return nil
	})
}

// ConsumeFuel consumes fuel.
//
// Custom filters, functions and methods can use this to account for
// expensive work they perform on behalf of a template.  The engine already
// charges for the size of the value they return, but not for the work that
// went into producing it.
//
// If this runs out of fuel, an error of kind ErrOutOfFuel is returned and the
// rendering will fail.  If fuel tracking is not enabled, this does nothing.
//
// Example:
//
//	env.AddFunction("expensive", func(state *State, args []value.Value, _ map[string]value.Value) (value.Value, error) {
//	    n, _ := args[0].AsInt()
//	    if err := state.ConsumeFuel(uint64(n)); err != nil {
//	        return value.Undefined(), err
//	    }
//	    return value.FromInt(expensiveComputation(n)), nil
//	})
func (s *State) ConsumeFuel(amount uint64) error {
	if s.fuelTracker == nil {
		return nil
	}
	units := amount
	if units > (1<<62)/fuel.UnitsPerFuel {
		units = 1 << 62
	} else {
		units *= fuel.UnitsPerFuel
	}
	return s.fuelTracker.Consume(units)
}

// consumeFuel charges for one evaluated statement or expression.
func (s *State) consumeFuel() error {
	return s.fuelTracker.Consume(fuel.InstructionCost)
}

// chargeFuel charges fuel in internal units.
func (s *State) chargeFuel(units uint64) error {
	return s.fuelTracker.Consume(units)
}

// chargeValue charges for the size of a value that was just created.
func (s *State) chargeValue(val value.Value) error {
	if s.fuelTracker == nil {
		return nil
	}
	return s.fuelTracker.Consume(fuel.ValueCost(val))
}

// chargeStringInput charges for the string a filter or method works on.
func (s *State) chargeStringInput(val value.Value) error {
	if s.fuelTracker == nil {
		return nil
	}
	if n, ok := fuel.StringLen(val); ok {
		return s.fuelTracker.Consume(fuel.CostOf(n, fuel.ByteCost))
	}
	return nil
}

// chargeOutput charges for bytes written to the output.
func (s *State) chargeOutput(n int) error {
	if s.fuelTracker == nil {
		return nil
	}
	return s.fuelTracker.Consume(fuel.CostOf(n, fuel.ByteCost))
}

// chargeSerialized charges the extra cost of serializing a value into n
// bytes.  The bytes themselves are charged separately.
func (s *State) chargeSerialized(n int) error {
	if s.fuelTracker == nil {
		return nil
	}
	return s.fuelTracker.Consume(fuel.CostOf(n, fuel.SerializedByteCost-fuel.ByteCost))
}

// ensureSlots checks that n list slots could still be paid for.
func (s *State) ensureSlots(n int) error {
	if s.fuelTracker == nil {
		return nil
	}
	return s.fuelTracker.Ensure(fuel.CostOf(n, fuel.SlotCost))
}

// iterItems returns the items of a value like value.Value.Iter, but charges
// for every item and stops iterating host objects once the fuel runs out.
func (s *State) iterItems(val value.Value) ([]value.Value, error) {
	return s.fuelTracker.Items(val)
}

// contains checks if a container contains an item and charges for the work.
func (s *State) contains(container, item value.Value) (bool, error) {
	return s.fuelTracker.Contains(container, item)
}

// isContainer returns true for values that are formatted by walking them.
func isContainer(val value.Value) bool {
	switch val.Kind() {
	case value.KindSeq, value.KindMap, value.KindIterable, value.KindPlain:
		return true
	}
	return false
}

// binopAllocEstimate estimates how many units the result of a binary
// operation will need.
//
// This only covers operations that create new strings or sequences and is
// used to refuse an allocation before it happens.  Unlike in Rust, repeating
// and concatenating lists materializes them in Go, so lists are counted too.
func binopAllocEstimate(op string, lhs, rhs value.Value) uint64 {
	strLen := func(v value.Value) (uint64, bool) {
		if n, ok := fuel.StringLen(v); ok {
			return uint64(n), true
		}
		if b, ok := v.Raw().([]byte); ok {
			return uint64(len(b)), true
		}
		return 0, false
	}
	seqLen := func(v value.Value) (int, bool) {
		switch v.Kind() {
		case value.KindSeq, value.KindIterable:
			return v.Len()
		}
		return 0, false
	}

	switch op {
	case "mul":
		repeated, count := lhs, rhs
		if _, ok := lhs.AsInt(); ok {
			repeated, count = rhs, lhs
		}
		n, ok := count.AsInt()
		if !ok || n <= 0 {
			return 0
		}
		if l, ok := strLen(repeated); ok {
			return fuel.CostOf(int(min(l, 1<<40)), uint64(n))
		}
		if l, ok := seqLen(repeated); ok {
			return fuel.CostOf(l, fuel.CostOf(int(min(n, 1<<40)), fuel.SlotCost))
		}
		return 0
	case "add", "concat":
		a, aok := strLen(lhs)
		b, bok := strLen(rhs)
		switch {
		case aok && bok:
			return a + b
		case aok && op == "concat":
			return a
		case bok && op == "concat":
			return b
		case op == "add":
			la, aok := seqLen(lhs)
			lb, bok := seqLen(rhs)
			if aok && bok {
				return fuel.CostOf(la+lb, fuel.SlotCost)
			}
		}
	}
	return 0
}

// ensureFuelForBinop checks that there is enough fuel left to perform a
// binary operation.
func (s *State) ensureFuelForBinop(op string, lhs, rhs value.Value) error {
	if s.fuelTracker == nil {
		return nil
	}
	return s.fuelTracker.Ensure(binopAllocEstimate(op, lhs, rhs))
}

// chargeFuelForCompare charges for comparing two values.
//
// Comparing containers needs to visit their items, so this charges for the
// items of the shorter one.
func (s *State) chargeFuelForCompare(lhs, rhs value.Value) error {
	if s.fuelTracker == nil || !isContainer(lhs) || !isContainer(rhs) || lhs.SameAs(rhs) {
		return nil
	}
	la, aok := lhs.Len()
	lb, bok := rhs.Len()
	if !aok || !bok {
		return nil
	}
	return s.fuelTracker.Consume(fuel.CostOf(min(la, lb), fuel.ItemCost))
}

// chargeFuelForGetItem charges for looking up an item by index in an
// iterable.  Iterables cannot be indexed, so the lookup needs to iterate up
// to the index.
//
// Strings need to be scanned to find a character, which is charged per
// byte scanned.
func (s *State) chargeFuelForGetItem(container, key value.Value) error {
	if s.fuelTracker == nil {
		return nil
	}
	idx, ok := key.AsInt()
	if !ok {
		return nil
	}
	if n, ok := fuel.StringLen(container); ok {
		if idx >= 0 && idx < int64(n) {
			// a character is at least one byte
			n = int(idx) + 1
		}
		return s.fuelTracker.Consume(fuel.CostOf(n, fuel.ByteCost))
	}
	if container.Kind() != value.KindIterable {
		return nil
	}
	if idx < 0 {
		idx = -idx
	}
	n := uint64(idx)
	if l, ok := container.Len(); ok && uint64(l) < n {
		n = uint64(l)
	}
	return s.fuelTracker.Consume(fuel.CostOf(int(min(n, 1<<40)), fuel.ItemCost))
}

// addValues adds two values.
//
// Concatenating host iterables materializes them, so they are iterated with
// fuel to make sure that infinite iterables cannot hang the engine.
func (s *State) addValues(left, right value.Value) (value.Value, error) {
	if s.fuelTracker != nil && !left.IsTuple() && !right.IsTuple() &&
		isSeqOrIterable(left) && isSeqOrIterable(right) &&
		(!fuel.IsMaterialized(left) || !fuel.IsMaterialized(right)) {
		a, err := s.iterItems(left)
		if err != nil {
			return value.Undefined(), err
		}
		b, err := s.iterItems(right)
		if err != nil {
			return value.Undefined(), err
		}
		if err := s.ensureSlots(len(a) + len(b)); err != nil {
			return value.Undefined(), err
		}
		result := make([]value.Value, 0, len(a)+len(b))
		result = append(result, a...)
		result = append(result, b...)
		return value.FromIterator(value.NewIterator("concat", result)), nil
	}
	return left.Add(right)
}

func isSeqOrIterable(v value.Value) bool {
	kind := v.Kind()
	return kind == value.KindSeq || kind == value.KindIterable
}
