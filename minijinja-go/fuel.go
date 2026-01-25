package minijinja

import (
	"math"
	"sync/atomic"
)

type fuelTracker struct {
	initial   uint64
	remaining atomic.Int64
}

func newFuelTracker(fuel uint64) *fuelTracker {
	if fuel > math.MaxInt64 {
		fuel = math.MaxInt64
	}
	tracker := &fuelTracker{initial: fuel}
	tracker.remaining.Store(int64(fuel))
	return tracker
}

func (f *fuelTracker) consume(amount int64) error {
	if amount == 0 {
		return nil
	}
	remaining := f.remaining.Add(-amount)
	if remaining <= 0 {
		return NewError(ErrOutOfFuel, "out of fuel")
	}
	return nil
}

func (f *fuelTracker) remainingFuel() uint64 {
	remaining := f.remaining.Load()
	if remaining <= 0 {
		return 0
	}
	return uint64(remaining)
}

func (f *fuelTracker) consumedFuel() uint64 {
	remaining := f.remainingFuel()
	if remaining >= f.initial {
		return 0
	}
	return f.initial - remaining
}
