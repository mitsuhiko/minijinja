package fuel

import (
	"errors"
	"testing"

	mjerrors "github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/errors"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

func isOutOfFuel(err error) bool {
	var mjErr *mjerrors.Error
	return errors.As(err, &mjErr) && mjErr.Kind == mjerrors.ErrOutOfFuel
}

func TestLevels(t *testing.T) {
	tracker := New(10)
	if err := tracker.Consume(UnitsPerFuel + 1); err != nil {
		t.Fatal(err)
	}
	// partially used fuel counts as consumed
	if tracker.Remaining() != 8 || tracker.Consumed() != 2 {
		t.Fatalf("unexpected levels %d/%d", tracker.Consumed(), tracker.Remaining())
	}
	if tracker.RemainingUnits() != 10*UnitsPerFuel-UnitsPerFuel-1 {
		t.Fatalf("unexpected units %d", tracker.RemainingUnits())
	}
}

func TestStickyExhaustion(t *testing.T) {
	tracker := New(1)
	if err := tracker.Consume(UnitsPerFuel); !isOutOfFuel(err) {
		t.Fatalf("expected out of fuel, got %v", err)
	}
	// once exhausted, every charge fails, even free ones
	if err := tracker.Consume(0); !isOutOfFuel(err) {
		t.Fatalf("expected out of fuel, got %v", err)
	}
	if err := tracker.Ensure(0); !isOutOfFuel(err) {
		t.Fatalf("expected out of fuel, got %v", err)
	}
	if tracker.Remaining() != 0 || tracker.Consumed() != 1 {
		t.Fatalf("unexpected levels %d/%d", tracker.Consumed(), tracker.Remaining())
	}
	// huge charges do not overflow the counter
	for i := 0; i < 4; i++ {
		if err := tracker.Consume(^uint64(0)); !isOutOfFuel(err) {
			t.Fatalf("expected out of fuel, got %v", err)
		}
	}
	if tracker.Remaining() != 0 {
		t.Fatalf("unexpected remaining %d", tracker.Remaining())
	}
}

func TestEnsure(t *testing.T) {
	tracker := New(10)
	if err := tracker.Ensure(10*UnitsPerFuel - 1); err != nil {
		t.Fatal(err)
	}
	// ensure does not consume
	if tracker.Remaining() != 10 {
		t.Fatalf("ensure consumed fuel: %d", tracker.Remaining())
	}
	// a quiet check fails without exhausting
	if err := tracker.EnsureQuiet(10 * UnitsPerFuel); !isOutOfFuel(err) {
		t.Fatalf("expected out of fuel, got %v", err)
	}
	if err := tracker.Consume(0); err != nil {
		t.Fatal(err)
	}
	// a failed ensure exhausts the tracker
	if err := tracker.Ensure(10 * UnitsPerFuel); !isOutOfFuel(err) {
		t.Fatalf("expected out of fuel, got %v", err)
	}
	if err := tracker.Consume(0); !isOutOfFuel(err) {
		t.Fatalf("expected out of fuel, got %v", err)
	}
}

func TestNilTracker(t *testing.T) {
	var tracker *Tracker
	if tracker.Consume(^uint64(0)) != nil || tracker.Ensure(^uint64(0)) != nil {
		t.Fatal("nil tracker should not fail")
	}
	items, err := tracker.Items(value.FromSlice([]value.Value{value.FromInt(1)}))
	if err != nil || len(items) != 1 {
		t.Fatalf("unexpected items %v %v", items, err)
	}
}

func TestCosts(t *testing.T) {
	if LookupCost(0) != 0 || LookupCost(1) != CompareCost || LookupCost(1000) != 10*CompareCost {
		t.Fatal("unexpected lookup cost")
	}
	if SortCost(1000) != 1000*10*CompareCost {
		t.Fatal("unexpected sort cost")
	}
	if CostOf(-1, 10) != 0 || CostOf(1<<62, 1<<10) != ^uint64(0) {
		t.Fatal("unexpected saturation")
	}
	cases := []struct {
		val  value.Value
		cost uint64
	}{
		{value.FromString("hello"), 5},
		{value.FromSafeString("hello"), 5},
		{value.FromBytes([]byte("abc")), 3},
		{value.FromInt(42), 0},
		{value.FromSlice(make([]value.Value, 3)), 3 * SlotCost},
		{value.FromTuple(make([]value.Value, 2)), 2 * SlotCost},
		{value.FromMap(map[string]value.Value{"a": value.None()}), 2 * SlotCost},
		{value.FromIterator(value.NewIterator("range", make([]value.Value, 4))), 4 * SlotCost},
	}
	for _, c := range cases {
		if got := ValueCost(c.val); got != c.cost {
			t.Errorf("cost of %s: got %d, want %d", c.val.Repr(), got, c.cost)
		}
	}
}
