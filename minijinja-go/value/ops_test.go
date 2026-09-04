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

func TestRemEuclidean(t *testing.T) {
	tests := []struct {
		name  string
		left  Value
		right Value
		want  string
	}{
		{"positive operands", FromInt(7), FromInt(3), "1"},
		{"negative dividend", FromInt(-7), FromInt(3), "2"},
		{"negative divisor", FromInt(7), FromInt(-3), "1"},
		{"both negative", FromInt(-7), FromInt(-3), "2"},
		{"exact division", FromInt(-9), FromInt(3), "0"},
		{"float keeps truncated remainder", FromFloat(-7.5), FromFloat(2), "-1.5"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			rv, err := test.left.Rem(test.right)
			if err != nil {
				t.Fatalf("modulo failed: %v", err)
			}
			if got := rv.String(); got != test.want {
				t.Fatalf("unexpected remainder: got %s, want %s", got, test.want)
			}
		})
	}
}

func TestRemByZero(t *testing.T) {
	if _, err := FromInt(1).Rem(FromInt(0)); err == nil {
		t.Fatal("expected modulo by zero to fail")
	}
}
