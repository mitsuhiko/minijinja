package value

import (
	"math"
	"testing"
)

func TestStringRepeatSizeLimit(t *testing.T) {
	tests := []struct {
		name  string
		left  Value
		right Value
	}{
		{"string times count", FromString("ab"), FromInt(50_000_001)},
		{"count times safe string", FromInt(50_000_001), FromSafeString("ab")},
		{"overflowing length", FromString("ab"), FromInt(math.MaxInt64)},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := test.left.Mul(test.right)
			if err == nil {
				t.Fatal("expected string repetition to fail")
			}
			if got, want := err.Error(), "repeated string is too large"; got != want {
				t.Fatalf("unexpected error: got %q, want %q", got, want)
			}
		})
	}
}

func TestEmptyStringLargeRepeat(t *testing.T) {
	rv, err := FromString("").Mul(FromInt(math.MaxInt64))
	if err != nil {
		t.Fatalf("empty string repetition failed: %v", err)
	}
	if got := rv.String(); got != "" {
		t.Fatalf("unexpected repeated string: %q", got)
	}
}

func TestFloorIntegerDivision(t *testing.T) {
	tests := []struct {
		name                string
		left, right         int64
		quotient, remainder int64
	}{
		{"positive operands", 7, 3, 2, 1},
		{"negative dividend", -7, 3, -3, 2},
		{"negative divisor", 7, -3, -3, -2},
		{"negative operands", -7, -3, 2, -1},
		{"exact division", -9, 3, -3, 0},
		{"large operands", math.MaxInt64, 3, 3074457345618258602, 1},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			quotient, err := FromInt(test.left).FloorDiv(FromInt(test.right))
			if err != nil {
				t.Fatalf("floor division failed: %v", err)
			}
			remainder, err := FromInt(test.left).Rem(FromInt(test.right))
			if err != nil {
				t.Fatalf("remainder failed: %v", err)
			}
			if got, ok := quotient.AsInt(); !ok || !quotient.IsActualInt() || got != test.quotient {
				t.Fatalf("unexpected quotient: got %v, want %d", quotient, test.quotient)
			}
			if got, ok := remainder.AsInt(); !ok || !remainder.IsActualInt() || got != test.remainder {
				t.Fatalf("unexpected remainder: got %v, want %d", remainder, test.remainder)
			}
			if test.quotient*test.right+test.remainder != test.left {
				t.Fatal("quotient and remainder do not reconstruct the dividend")
			}
		})
	}
}

func TestFloorIntegerDivisionOverflow(t *testing.T) {
	quotient, err := FromInt(math.MinInt64).FloorDiv(FromInt(-1))
	if err != nil {
		t.Fatalf("floor division failed: %v", err)
	}
	if got, want := quotient.String(), "9223372036854775808"; got != want {
		t.Fatalf("unexpected quotient: got %s, want %s", got, want)
	}
	remainder, err := FromInt(math.MinInt64).Rem(FromInt(-1))
	if err != nil {
		t.Fatalf("remainder failed: %v", err)
	}
	if got, _ := remainder.AsInt(); got != 0 {
		t.Fatalf("unexpected remainder: got %v, want 0", remainder)
	}
}

func TestFloatDivisionAndRemainder(t *testing.T) {
	tests := []struct {
		name             string
		left, right      Value
		quotient, remain string
	}{
		{"negative dividend", FromFloat(-7), FromFloat(3), "-3.0", "2.0"},
		{"negative divisor", FromFloat(7), FromFloat(-3), "-3.0", "-2.0"},
		{"negative operands", FromFloat(-7), FromFloat(-3), "2.0", "-1.0"},
		{"fractional operands", FromFloat(-7.5), FromFloat(2), "-4.0", "0.5"},
		{"mixed operands", FromInt(-7), FromFloat(3), "-3.0", "2.0"},
		{"rounding correction", FromFloat(1), FromFloat(0.1), "9.0", "0.09999999999999995"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			quotient, err := test.left.FloorDiv(test.right)
			if err != nil {
				t.Fatalf("floor division failed: %v", err)
			}
			remainder, err := test.left.Rem(test.right)
			if err != nil {
				t.Fatalf("remainder failed: %v", err)
			}
			if got := quotient.String(); got != test.quotient {
				t.Fatalf("unexpected quotient: got %s, want %s", got, test.quotient)
			}
			if got := remainder.String(); got != test.remain {
				t.Fatalf("unexpected remainder: got %s, want %s", got, test.remain)
			}
		})
	}
}

func TestDivisionByZero(t *testing.T) {
	if _, err := FromInt(1).FloorDiv(FromInt(0)); err == nil {
		t.Fatal("expected integer floor division by zero to fail")
	}
	if _, err := FromInt(1).Rem(FromInt(0)); err == nil {
		t.Fatal("expected integer remainder by zero to fail")
	}

	if _, err := FromFloat(1).FloorDiv(FromFloat(0)); err == nil {
		t.Fatal("expected float floor division by zero to fail")
	}
	if _, err := FromFloat(1).Rem(FromFloat(0)); err == nil {
		t.Fatal("expected float remainder by zero to fail")
	}
}
