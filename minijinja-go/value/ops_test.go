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
