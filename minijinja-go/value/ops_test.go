package value

import (
	"math"
	"math/big"
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

func TestIntegerArithmeticIsExact(t *testing.T) {
	const beyondFloat = 1<<53 + 1
	tests := []struct {
		name string
		op   func(Value, Value) (Value, error)
		a, b Value
		want string
	}{
		{"add beyond float precision", Value.Add, FromInt(beyondFloat), FromInt(0), "9007199254740993"},
		{"add bool", Value.Add, FromInt(beyondFloat), FromBool(true), "9007199254740994"},
		{"sub beyond float precision", Value.Sub, FromInt(beyondFloat), FromInt(0), "9007199254740993"},
		{"mul beyond float precision", Value.Mul, FromInt(beyondFloat), FromInt(1), "9007199254740993"},
		{"add overflow", Value.Add, FromInt(math.MaxInt64), FromInt(1), "9223372036854775808"},
		{"sub overflow", Value.Sub, FromInt(math.MinInt64), FromInt(1), "-9223372036854775809"},
		{"sub overflow with negative operand", Value.Sub, FromInt(math.MaxInt64), FromInt(-1), "9223372036854775808"},
		{"mul overflow", Value.Mul, FromInt(math.MaxInt64), FromInt(2), "18446744073709551614"},
		{"mul overflow of the minimum", Value.Mul, FromInt(math.MinInt64), FromInt(-1), "9223372036854775808"},
		{"mul small negative", Value.Mul, FromInt(-3), FromInt(7), "-21"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			rv, err := test.op(test.a, test.b)
			if err != nil {
				t.Fatalf("operation failed: %v", err)
			}
			if got := rv.String(); got != test.want {
				t.Fatalf("got %s, want %s", got, test.want)
			}
		})
	}
}

func TestBigIntegerArithmetic(t *testing.T) {
	bigInt := func(s string) Value {
		n, ok := new(big.Int).SetString(s, 10)
		if !ok {
			t.Fatalf("invalid big integer %q", s)
		}
		return FromBigInt(n)
	}
	maxI128 := "170141183460469231731687303715884105727"
	minI128 := "-170141183460469231731687303715884105728"

	tests := []struct {
		name string
		op   func(Value, Value) (Value, error)
		a, b Value
		want string
	}{
		{"add", Value.Add, bigInt("9223372036854775808"), FromInt(1), "9223372036854775809"},
		{"add bool", Value.Add, FromBool(true), bigInt("9223372036854775808"), "9223372036854775809"},
		{"add back into int64", Value.Add, bigInt("9223372036854775808"), FromInt(-1), "9223372036854775807"},
		{"sub", Value.Sub, bigInt("9223372036854775808"), bigInt("9223372036854775809"), "-1"},
		{"mul", Value.Mul, bigInt("9223372036854775808"), FromInt(2), "18446744073709551616"},
		{"mul float", Value.Mul, bigInt("9223372036854775808"), FromFloat(2.5), "23058430092136940000.0"},
		{"div", Value.Div, bigInt("9223372036854775808"), FromInt(2), "4611686018427388000.0"},
		{"floor div", Value.FloorDiv, bigInt("-9223372036854775809"), FromInt(2), "-4611686018427387905"},
		{"rem", Value.Rem, bigInt("-9223372036854775809"), FromInt(10), "1"},
		{"floor div smallest i128 by -1", Value.FloorDiv, bigInt(minI128), FromInt(-1), "170141183460469231731687303715884105728"},
		{"rem smallest i128 by -1", Value.Rem, bigInt(minI128), FromInt(-1), "0"},
		{"float rem", Value.Rem, FromFloat(1000), bigInt("18446744073709551616"), "1000.0"},
		{"pow", Value.Pow, bigInt("9223372036854775808"), FromInt(2), "85070591730234615865843651857942052864"},
		{"pow negative exponent", Value.Pow, FromInt(1000), bigInt("-9223372036854775808"), "0.0"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			rv, err := test.op(test.a, test.b)
			if err != nil {
				t.Fatalf("operation failed: %v", err)
			}
			if got := rv.String(); got != test.want {
				t.Fatalf("got %s, want %s", got, test.want)
			}
		})
	}

	// Like the i128 math in Rust, results outside the i128 range fail.
	failures := []struct {
		name string
		op   func(Value, Value) (Value, error)
		a, b Value
	}{
		{"add overflow", Value.Add, bigInt(maxI128), FromInt(1)},
		{"sub overflow", Value.Sub, bigInt(minI128), FromInt(1)},
		{"mul overflow", Value.Mul, bigInt(maxI128), FromInt(2)},
		{"floor div operand overflow", Value.FloorDiv, bigInt("170141183460469231731687303715884105728"), FromInt(1)},
		{"rem operand overflow", Value.Rem, bigInt("170141183460469231731687303715884105728"), FromInt(3)},
		{"floor div by zero", Value.FloorDiv, bigInt("9223372036854775808"), FromInt(0)},
		{"rem by zero", Value.Rem, bigInt("9223372036854775808"), FromInt(0)},
		{"pow overflow", Value.Pow, FromInt(2), bigInt("9223372036854775808")},
		{"pow exponent beyond u32", Value.Pow, FromInt(-1), FromInt(1 << 32)},
	}
	for _, test := range failures {
		t.Run(test.name, func(t *testing.T) {
			if rv, err := test.op(test.a, test.b); err == nil {
				t.Fatalf("expected an error, got %s", rv)
			}
		})
	}
}
