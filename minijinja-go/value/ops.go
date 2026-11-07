package value

import (
	"bytes"
	"cmp"
	"fmt"
	"math"
	"math/big"
	"strings"
)

var minI128AsU128 = new(big.Int).Lsh(big.NewInt(1), 127)

const (
	maxRepeatedStringLen   = 100_000_000
	maxRepeatedSequenceLen = 100_000_000
)

// Neg performs unary negation.
func (v Value) Neg() (Value, error) {
	switch d := v.data.(type) {
	case int64:
		return FromInt(-d), nil
	case bigIntValue:
		if d.Int.Sign() >= 0 && d.Int.Cmp(minI128AsU128) == 0 {
			return FromBigInt(new(big.Int).Set(d.Int)), nil
		}
		result := new(big.Int).Neg(d.Int)
		return FromBigInt(result), nil
	case float64:
		return FromFloat(-d), nil
	default:
		return Undefined(), fmt.Errorf("cannot negate %s", v.Kind())
	}
}

// compareBigNumbers compares two numbers if at least one of them is a big
// integer.  float64 can't represent big integers exactly, so integers are
// compared as big integers and floats as big floats.  The second return value
// is false if neither value is a big integer or the values can't be compared.
func compareBigNumbers(a, b Value) (int, bool) {
	_, aBig := a.data.(bigIntValue)
	_, bBig := b.data.(bigIntValue)
	if !aBig && !bBig {
		return 0, false
	}
	toBigFloat := func(v Value) (*big.Float, bool) {
		switch d := v.data.(type) {
		case int64:
			return new(big.Float).SetInt64(d), true
		case bigIntValue:
			return new(big.Float).SetInt(d.Int), true
		case float64:
			if math.IsNaN(d) {
				return nil, false
			}
			return big.NewFloat(d), true
		}
		return nil, false
	}
	x, ok := toBigFloat(a)
	if !ok {
		return 0, false
	}
	y, ok := toBigFloat(b)
	if !ok {
		return 0, false
	}
	return x.Cmp(y), true
}

// isActualInt returns true if the value is stored as an int64, not a float64.
func isActualInt(v Value) bool {
	_, ok := v.data.(int64)
	return ok
}

func asArithmeticFloat(v Value) (float64, bool) {
	if b, ok := v.AsBool(); ok {
		return boolToFloat(b), true
	}
	if d, ok := v.data.(bigIntValue); ok {
		f, _ := new(big.Float).SetInt(d.Int).Float64()
		return f, true
	}
	return v.AsFloat()
}

func isArithmeticInt(v Value) bool {
	return isActualInt(v) || isBigInt(v) || v.Kind() == KindBool
}

func isBigInt(v Value) bool {
	_, ok := v.data.(bigIntValue)
	return ok
}

// arithmeticBigInt returns an integer operand (an integer, a big integer or a
// boolean) as a big.Int.
func arithmeticBigInt(v Value) *big.Int {
	if d, ok := v.data.(bigIntValue); ok {
		return d.Int
	}
	return big.NewInt(arithmeticInt(v))
}

var maxI128 = new(big.Int).Sub(minI128AsU128, big.NewInt(1))
var minI128 = new(big.Int).Neg(minI128AsU128)

// fitsI128 reports whether an integer is in the range of the i128 integers
// that Rust does integer math with.
func fitsI128(x *big.Int) bool {
	return x.Cmp(minI128) >= 0 && x.Cmp(maxI128) <= 0
}

func bigIntResult(x *big.Int) Value {
	if x.IsInt64() {
		return FromInt(x.Int64())
	}
	return FromBigInt(x)
}

func arithmeticInt(v Value) int64 {
	if b, ok := v.AsBool(); ok {
		return int64(boolToFloat(b))
	}
	i, _ := v.AsInt()
	return i
}

func checkedAdd(a, b int64) (int64, bool) {
	r := a + b
	return r, (r > a) == (b > 0)
}

func checkedSub(a, b int64) (int64, bool) {
	r := a - b
	return r, (r < a) == (b > 0)
}

func checkedMul(a, b int64) (int64, bool) {
	if a == 0 || b == 0 {
		return 0, true
	}
	r := a * b
	return r, r/b == a && !(a == math.MinInt64 && b == -1)
}

// intArith applies an operation to two integer operands.  Unlike float64 math
// this is exact, and like the i128 math in Rust a result that doesn't fit an
// int64 becomes a big integer instead of overflowing, while operands or
// results outside the i128 range fail.
func intArith(v, other Value, opName string, checked func(a, b int64) (int64, bool), op func(z, x, y *big.Int) *big.Int) (Value, error) {
	if !isBigInt(v) && !isBigInt(other) {
		if result, ok := checked(arithmeticInt(v), arithmeticInt(other)); ok {
			return FromInt(result), nil
		}
	}
	a, b := arithmeticBigInt(v), arithmeticBigInt(other)
	result := op(new(big.Int), a, b)
	if !fitsI128(a) || !fitsI128(b) || !fitsI128(result) {
		return Undefined(), fmt.Errorf("unable to calculate %s %s %s", v.Repr(), opName, other.Repr())
	}
	return bigIntResult(result), nil
}

// Add performs addition or string concatenation.
func (v Value) Add(other Value) (Value, error) {
	// String concatenation
	if s1, ok := v.AsString(); ok {
		if s2, ok := other.AsString(); ok {
			if v.IsSafe() || other.IsSafe() {
				return FromSafeString(s1 + s2), nil
			}
			return FromString(s1 + s2), nil
		}
	}

	// Numeric addition
	if f1, ok := asArithmeticFloat(v); ok {
		if f2, ok := asArithmeticFloat(other); ok {
			// Booleans are integers for arithmetic, matching Python.
			if isArithmeticInt(v) && isArithmeticInt(other) {
				return intArith(v, other, "+", checkedAdd, (*big.Int).Add)
			}
			return FromFloat(f1 + f2), nil
		}
	}

	// Sequence concatenation.  Like Python, tuples only concatenate with tuples.
	if v.IsTuple() || other.IsTuple() {
		if v.IsTuple() && other.IsTuple() {
			s1, _ := v.AsSlice()
			s2, _ := other.AsSlice()
			result := make([]Value, 0, len(s1)+len(s2))
			result = append(result, s1...)
			result = append(result, s2...)
			return FromTuple(result), nil
		}
		return Undefined(), fmt.Errorf("cannot add %s and %s", v.Kind(), other.Kind())
	}
	if s1, ok := v.AsSlice(); ok {
		if s2, ok := other.AsSlice(); ok {
			result := make([]Value, 0, len(s1)+len(s2))
			result = append(result, s1...)
			result = append(result, s2...)
			return FromSlice(result), nil
		}
	}

	// Like in Rust, a sequence or iterable such as range() also concatenates
	// with another one, which gives an iterable.
	if isSeqOrIterable(v) && isSeqOrIterable(other) {
		s1 := v.Iter()
		s2 := other.Iter()
		result := make([]Value, 0, len(s1)+len(s2))
		result = append(result, s1...)
		result = append(result, s2...)
		return FromIterator(NewIterator("concat", result)), nil
	}

	return Undefined(), fmt.Errorf("cannot add %s and %s", v.Kind(), other.Kind())
}

// Sub performs subtraction.
func (v Value) Sub(other Value) (Value, error) {
	if f1, ok := asArithmeticFloat(v); ok {
		if f2, ok := asArithmeticFloat(other); ok {
			// Booleans are integers for arithmetic, matching Python.
			if isArithmeticInt(v) && isArithmeticInt(other) {
				return intArith(v, other, "-", checkedSub, (*big.Int).Sub)
			}
			return FromFloat(f1 - f2), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot subtract %s from %s", other.Kind(), v.Kind())
}

// repeatCount returns how often a string or sequence is repeated.  Like in
// Python, booleans count as integers and a negative count repeats zero times.
func repeatCount(v Value) (int64, bool) {
	if b, ok := v.AsBool(); ok {
		return int64(boolToFloat(b)), true
	}
	if b, ok := v.data.(bigIntValue); ok && b.Sign() < 0 {
		return 0, true
	}
	// Like Rust, floats only count if they are integral and fit into an
	// int64.  Converting larger floats to int64 is not well defined.
	if f, ok := v.data.(float64); ok && (f < -(1<<63) || f >= 1<<63) {
		return 0, false
	}
	n, ok := v.AsInt()
	if ok && n < 0 {
		n = 0
	}
	return n, ok
}

func repeatString(s string, n int64, safe bool) (Value, error) {
	if len(s) > 0 && n > int64(maxRepeatedStringLen/len(s)) {
		return Undefined(), fmt.Errorf("repeated string is too large")
	}

	var repeated string
	if len(s) > 0 {
		repeated = strings.Repeat(s, int(n))
	}
	if safe {
		return FromSafeString(repeated), nil
	}
	return FromString(repeated), nil
}

// Mul performs multiplication.
func (v Value) Mul(other Value) (Value, error) {
	// String repetition
	if s, ok := v.AsString(); ok {
		if n, ok := repeatCount(other); ok {
			return repeatString(s, n, v.IsSafe())
		}
	}
	if n, ok := repeatCount(v); ok {
		if s, ok := other.AsString(); ok {
			return repeatString(s, n, other.IsSafe())
		}
	}

	// Sequence/Iterator repetition (seq * n)
	if n, ok := repeatCount(other); ok {
		if v.Kind() == KindSeq || v.Kind() == KindIterable {
			return repeatIterable(v, n)
		}
	}
	// Sequence/Iterator repetition (n * seq)
	if n, ok := repeatCount(v); ok {
		if other.Kind() == KindSeq || other.Kind() == KindIterable {
			return repeatIterable(other, n)
		}
	}

	// Numeric multiplication
	if f1, ok := asArithmeticFloat(v); ok {
		if f2, ok := asArithmeticFloat(other); ok {
			// Booleans are integers for arithmetic, matching Python.
			if isArithmeticInt(v) && isArithmeticInt(other) {
				return intArith(v, other, "*", checkedMul, (*big.Int).Mul)
			}
			return FromFloat(f1 * f2), nil
		}
	}

	return Undefined(), fmt.Errorf("cannot multiply %s and %s", v.Kind(), other.Kind())
}

func repeatIterable(seq Value, n int64) (Value, error) {
	length, ok := seq.Len()
	if !ok {
		return Undefined(), fmt.Errorf("cannot repeat unsized iterables")
	}
	if length == 0 || n == 0 {
		if seq.IsTuple() {
			return FromTuple(nil), nil
		}
		return FromSlice(nil), nil
	}
	if n > int64(maxRepeatedSequenceLen/length) {
		return Undefined(), fmt.Errorf("repeated sequence is too large")
	}

	items := seq.Iter()
	if items == nil {
		return Undefined(), fmt.Errorf("cannot repeat unsized iterables")
	}

	result := make([]Value, 0, length*int(n))
	for i := int64(0); i < n; i++ {
		result = append(result, items...)
	}
	if seq.IsTuple() {
		return FromTuple(result), nil
	}
	return FromSlice(result), nil
}

// Div performs division.
func (v Value) Div(other Value) (Value, error) {
	if f1, ok := asArithmeticFloat(v); ok {
		if f2, ok := asArithmeticFloat(other); ok {
			if f2 == 0 {
				return Undefined(), fmt.Errorf("division by zero")
			}
			return FromFloat(f1 / f2), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot divide %s by %s", v.Kind(), other.Kind())
}

func intDivRemFloor(a, b int64) (quotient, remainder int64) {
	quotient, remainder = a/b, a%b
	if remainder != 0 && (remainder < 0) != (b < 0) {
		quotient--
		remainder += b
	}
	return quotient, remainder
}

// bigIntDivRemFloor is intDivRemFloor for big integers.
func bigIntDivRemFloor(a, b *big.Int) (quotient, remainder *big.Int) {
	quotient, remainder = new(big.Int).QuoRem(a, b, new(big.Int))
	if remainder.Sign() != 0 && (remainder.Sign() < 0) != (b.Sign() < 0) {
		quotient.Sub(quotient, big.NewInt(1))
		remainder.Add(remainder, b)
	}
	return quotient, remainder
}

func floatDivRemFloor(a, b float64) (quotient, remainder float64) {
	remainder = math.Mod(a, b)
	quotient = (a - remainder) / b
	if remainder != 0 {
		if (remainder < 0) != (b < 0) {
			remainder += b
			quotient--
		}
	} else {
		remainder = math.Copysign(0, b)
	}
	if quotient != 0 {
		floored := math.Floor(quotient)
		if quotient-floored > 0.5 {
			floored++
		}
		quotient = floored
	} else {
		quotient = math.Copysign(0, a/b)
	}
	return quotient, remainder
}

// FloorDiv performs floor division.
func (v Value) FloorDiv(other Value) (Value, error) {
	if isBigInt(v) || isBigInt(other) {
		if isArithmeticInt(v) && isArithmeticInt(other) {
			return bigIntDivRem(v, other, "//")
		}
	} else if isArithmeticInt(v) && isArithmeticInt(other) {
		i1, i2 := arithmeticInt(v), arithmeticInt(other)
		if i2 == 0 {
			return Undefined(), fmt.Errorf("division by zero")
		}
		if i1 == math.MinInt64 && i2 == -1 {
			return FromBigInt(new(big.Int).Neg(big.NewInt(i1))), nil
		}
		quotient, _ := intDivRemFloor(i1, i2)
		return FromInt(quotient), nil
	}
	if f1, ok := asArithmeticFloat(v); ok {
		if f2, ok := asArithmeticFloat(other); ok {
			if f2 == 0 {
				return Undefined(), fmt.Errorf("division by zero")
			}
			quotient, _ := floatDivRemFloor(f1, f2)
			return FromFloat(quotient), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot floor divide %s by %s", v.Kind(), other.Kind())
}

// Rem performs a modulo operation with the sign of the divisor.
func (v Value) Rem(other Value) (Value, error) {
	if isBigInt(v) || isBigInt(other) {
		if isArithmeticInt(v) && isArithmeticInt(other) {
			return bigIntDivRem(v, other, "%")
		}
	} else if isArithmeticInt(v) && isArithmeticInt(other) {
		i1, i2 := arithmeticInt(v), arithmeticInt(other)
		if i2 == 0 {
			return Undefined(), fmt.Errorf("modulo by zero")
		}
		_, remainder := intDivRemFloor(i1, i2)
		return FromInt(remainder), nil
	}
	if f1, ok := asArithmeticFloat(v); ok {
		if f2, ok := asArithmeticFloat(other); ok {
			if f2 == 0 {
				return Undefined(), fmt.Errorf("modulo by zero")
			}
			_, remainder := floatDivRemFloor(f1, f2)
			return FromFloat(remainder), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot modulo %s by %s", v.Kind(), other.Kind())
}

// bigIntDivRem performs floor division (`//`) or modulo (`%`) where at least
// one integer operand is a big integer.  Like in Rust, the operands have to
// fit an i128, but the smallest i128 floor divided by -1 gives 2**127 and the
// matching remainder is 0.
func bigIntDivRem(v, other Value, opName string) (Value, error) {
	a, b := arithmeticBigInt(v), arithmeticBigInt(other)
	if !fitsI128(a) || !fitsI128(b) {
		return Undefined(), fmt.Errorf("unable to calculate %s %s %s", v.Repr(), opName, other.Repr())
	}
	if b.Sign() == 0 {
		if opName == "%" {
			return Undefined(), fmt.Errorf("modulo by zero")
		}
		return Undefined(), fmt.Errorf("division by zero")
	}
	quotient, remainder := bigIntDivRemFloor(a, b)
	result := quotient
	if opName == "%" {
		result = remainder
	}
	return bigIntResult(result), nil
}

// Pow performs exponentiation.
func (v Value) Pow(other Value) (Value, error) {
	f1, ok1 := asArithmeticFloat(v)
	f2, ok2 := asArithmeticFloat(other)
	if !ok1 || !ok2 {
		return Undefined(), fmt.Errorf("cannot compute power of %s and %s", v.Kind(), other.Kind())
	}
	// Zero to a negative power fails like a division by zero.
	if f1 == 0 && f2 < 0 {
		return Undefined(), fmt.Errorf("unable to calculate %s ** %s", v.Repr(), other.Repr())
	}
	// Integers to a non-negative integer power stay exact integers.  A
	// negative exponent gives a float, like in Python.
	if isArithmeticInt(v) && isArithmeticInt(other) && f2 >= 0 {
		if result, ok := intPow(arithmeticBigInt(v), arithmeticBigInt(other)); ok {
			return result, nil
		}
		return Undefined(), fmt.Errorf("unable to calculate %s ** %s", v.Repr(), other.Repr())
	}
	return FromFloat(math.Pow(f1, f2)), nil
}

// intPow raises an integer to a non-negative integer power.  Like the i128
// math in Rust, it fails if the result does not fit into 128 bits.
func intPow(base, exp *big.Int) (Value, bool) {
	if !fitsI128(base) || !fitsI128(exp) {
		return Value{}, false
	}
	// Like in Rust, the exponent has to fit into a u32.
	if exp.Cmp(big.NewInt(math.MaxUint32)) > 0 {
		return Value{}, false
	}
	if base.CmpAbs(big.NewInt(1)) > 0 && exp.Cmp(big.NewInt(127)) > 0 {
		return Value{}, false
	}
	result := new(big.Int).Exp(base, exp, nil)
	if !fitsI128(result) {
		return Value{}, false
	}
	return bigIntResult(result), true
}

// boolToFloat converts a bool to float for coercion (false=0, true=1)
func boolToFloat(b bool) float64 {
	if b {
		return 1.0
	}
	return 0.0
}

// intEqualsFloat reports whether an integer and a float are the same
// number.
func intEqualsFloat(i int64, f float64) bool {
	return f == math.Trunc(f) && f >= -(1<<63) && f < 1<<63 && int64(f) == i
}

func boolToInt(b bool) int64 {
	if b {
		return 1
	}
	return 0
}

// Equal returns true if two values are equal.
func (v Value) Equal(other Value) bool {
	// Undefined is only equal to undefined
	if v.IsUndefined() || other.IsUndefined() {
		return v.IsUndefined() && other.IsUndefined()
	}

	// None is only equal to none
	if v.IsNone() || other.IsNone() {
		return v.IsNone() && other.IsNone()
	}

	// Bool comparison (including coercion with numbers)
	if b1, ok := v.AsBool(); ok {
		if b2, ok := other.AsBool(); ok {
			return b1 == b2
		}
		// Bool vs number: coerce bool to float
		if f2, ok := other.AsFloat(); ok {
			return boolToFloat(b1) == f2
		}
		return false
	}
	// Number vs bool
	if f1, ok := v.AsFloat(); ok {
		if b2, ok := other.AsBool(); ok {
			return f1 == boolToFloat(b2)
		}
	}

	// Numeric comparison
	if cmp, ok := compareBigNumbers(v, other); ok {
		return cmp == 0
	}
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			// Like in Rust and Python, integers and floats are only equal
			// if they are the same number, without rounding the integer.
			i1, isInt1 := v.data.(int64)
			i2, isInt2 := other.data.(int64)
			switch {
			case isInt1 && isInt2:
				return i1 == i2
			case isInt1:
				return intEqualsFloat(i1, f2)
			case isInt2:
				return intEqualsFloat(i2, f1)
			}
			return f1 == f2
		}
		return false
	}

	// Bytes comparison
	if b1, ok := v.data.([]byte); ok {
		if b2, ok := other.data.([]byte); ok {
			return bytes.Equal(b1, b2)
		}
		return false
	}

	// String comparison
	if s1, ok := v.AsString(); ok {
		if s2, ok := other.AsString(); ok {
			return s1 == s2
		}
		return false
	}

	// Objects with a custom comparison decide on their own.
	if o1, ok := v.data.(Object); ok {
		if o2, ok := other.data.(Object); ok {
			if cmp, ok := CompareObjects(o1, o2); ok {
				return cmp == 0
			}
		}
	}

	// Sequence comparison.  Like in Rust, sequences and iterables compare
	// equal if they produce the same items.
	if isSeqOrIterable(v) && isSeqOrIterable(other) {
		if v.IsTuple() != other.IsTuple() {
			return false
		}
		seq1, seq2 := v.Iter(), other.Iter()
		if len(seq1) != len(seq2) {
			return false
		}
		for i := range seq1 {
			if !seq1[i].Equal(seq2[i]) {
				return false
			}
		}
		return true
	}

	// Map comparison
	if m1, ok := v.data.(map[string]Value); ok {
		if m2, ok := other.data.(map[string]Value); ok {
			if len(m1) != len(m2) {
				return false
			}
			for k, val1 := range m1 {
				if val2, exists := m2[k]; !exists || !val1.Equal(val2) {
					return false
				}
			}
			return true
		}
	}
	if m1, ok := v.AsValueMap(); ok {
		if m2, ok := other.AsValueMap(); ok {
			if m1.Len() != m2.Len() {
				return false
			}
			for k, val1 := range m1.All() {
				if val2, exists := m2.Get(k); !exists || !val1.Equal(val2) {
					return false
				}
			}
			return true
		}
		return false
	}

	return false
}

// Compare returns -1 if v < other, 0 if equal, 1 if v > other.
// Unlike Equal, Compare uses kind ordering first (like Rust's Ord), except
// that bools compare with numbers by value.
func (v Value) Compare(other Value) (int, bool) {
	if b, ok := v.data.(bool); ok && other.Kind() == KindNumber {
		return FromInt(boolToInt(b)).Compare(other)
	}
	if b, ok := other.data.(bool); ok && v.Kind() == KindNumber {
		return v.Compare(FromInt(boolToInt(b)))
	}

	// Compare by kind first (like Rust)
	// Kind order: Undefined < None < Bool < Number < String < Bytes < Seq < Map
	kindOrder := func(k ValueKind) int {
		switch k {
		case KindUndefined:
			return 0
		case KindNone:
			return 1
		case KindBool:
			return 2
		case KindNumber:
			return 3
		case KindString:
			return 4
		case KindBytes:
			return 5
		case KindSeq:
			return 6
		case KindMap:
			return 7
		default:
			return 8
		}
	}

	k1, k2 := kindOrder(v.Kind()), kindOrder(other.Kind())
	if k1 < k2 {
		return -1, true
	}
	if k1 > k2 {
		return 1, true
	}

	// Same kind - do value comparison
	// Bool comparison
	if b1, ok := v.AsBool(); ok {
		if b2, ok := other.AsBool(); ok {
			i1, i2 := 0, 0
			if b1 {
				i1 = 1
			}
			if b2 {
				i2 = 1
			}
			if i1 < i2 {
				return -1, true
			}
			if i1 > i2 {
				return 1, true
			}
			return 0, true
		}
	}

	// Numeric comparison
	if cmp, ok := compareBigNumbers(v, other); ok {
		return cmp, true
	}
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			if f1 < f2 {
				return -1, true
			}
			if f1 > f2 {
				return 1, true
			}
			return 0, true
		}
	}

	// String comparison
	if s1, ok := v.AsString(); ok {
		if s2, ok := other.AsString(); ok {
			if s1 < s2 {
				return -1, true
			}
			if s1 > s2 {
				return 1, true
			}
			return 0, true
		}
	}

	// Sequence comparison (lexicographic).  Tuples and lists sort separately.
	if seq1, ok := v.AsSlice(); ok {
		if seq2, ok := other.AsSlice(); ok {
			if v.IsTuple() != other.IsTuple() {
				if v.IsTuple() {
					return 1, true
				}
				return -1, true
			}
			minLen := len(seq1)
			if len(seq2) < minLen {
				minLen = len(seq2)
			}
			for i := 0; i < minLen; i++ {
				if cmp, ok := seq1[i].Compare(seq2[i]); ok && cmp != 0 {
					return cmp, true
				}
			}
			if len(seq1) < len(seq2) {
				return -1, true
			}
			if len(seq1) > len(seq2) {
				return 1, true
			}
			return 0, true
		}
	}

	// Object comparison using ObjectWithCmp
	if obj1, ok := v.AsObject(); ok {
		if obj2, ok := other.AsObject(); ok {
			if cmp, ok := CompareObjects(obj1, obj2); ok {
				return cmp, true
			}
		}
	}

	// Map comparison.  Like in Rust, the (key, value) pairs are compared in
	// key order.
	if m1, ok := v.AsValueMap(); ok {
		if m2, ok := other.AsValueMap(); ok {
			return compareMaps(m1, m2), true
		}
	}

	return 0, false
}

// compareMaps compares two maps by their (key, value) pairs in key order.
// Keys are ordered as values, so {9: 1} < {10: 1}.
func compareMaps(m1, m2 *Map) int {
	pairs1 := m1.sortedPairs()
	pairs2 := m2.sortedPairs()
	for i := 0; i < len(pairs1) && i < len(pairs2); i++ {
		if cmp, ok := pairs1[i][0].Compare(pairs2[i][0]); ok && cmp != 0 {
			return cmp
		}
		if cmp, ok := pairs1[i][1].Compare(pairs2[i][1]); ok && cmp != 0 {
			return cmp
		}
	}
	return cmp.Compare(len(pairs1), len(pairs2))
}

// Contains checks if v contains other.
func (v Value) Contains(other Value) bool {
	switch d := v.data.(type) {
	// Like in Rust, a value that is not a string is looked up by its
	// string representation.
	case string:
		return strings.Contains(d, other.String())
	case safeString:
		return strings.Contains(string(d), other.String())
	case []Value:
		for _, item := range d {
			if item.Equal(other) {
				return true
			}
		}
	case tupleValue:
		for _, item := range d {
			if item.Equal(other) {
				return true
			}
		}
	case *Iterator:
		for _, item := range d.items {
			if item.Equal(other) {
				return true
			}
		}
	case map[string]Value:
		if s, ok := other.AsString(); ok {
			_, exists := d[s]
			return exists
		}
	case *Map:
		_, exists := d.Get(other)
		return exists
	case Object:
		// Like in Rust, map objects are checked for the key while
		// sequences and iterables are checked for the item.
		switch v.Kind() {
		case KindMap:
			return !v.GetItem(other).IsUndefined()
		case KindSeq, KindIterable:
			for _, item := range v.Iter() {
				if item.Equal(other) {
					return true
				}
			}
		}
	}
	return false
}

func isSeqOrIterable(v Value) bool {
	kind := v.Kind()
	return kind == KindSeq || kind == KindIterable
}

// Concat performs the tilde (~) string concatenation.
func (v Value) Concat(other Value) Value {
	s1 := v.String()
	s2 := other.String()
	if v.IsSafe() && other.IsSafe() {
		return FromSafeString(s1 + s2)
	}
	return FromString(s1 + s2)
}
