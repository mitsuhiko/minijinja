package value

import (
	"fmt"
	"math"
	"math/big"
	"strings"
)

var minI128AsU128 = new(big.Int).Lsh(big.NewInt(1), 127)

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

// isActualInt returns true if the value is stored as an int64, not a float64.
func isActualInt(v Value) bool {
	_, ok := v.data.(int64)
	return ok
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
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			// Return int only if both are actual ints (not floats)
			if isActualInt(v) && isActualInt(other) {
				return FromInt(int64(f1 + f2)), nil
			}
			return FromFloat(f1 + f2), nil
		}
	}

	// Sequence concatenation
	if s1, ok := v.AsSlice(); ok {
		if s2, ok := other.AsSlice(); ok {
			result := make([]Value, 0, len(s1)+len(s2))
			result = append(result, s1...)
			result = append(result, s2...)
			return FromSlice(result), nil
		}
	}

	return Undefined(), fmt.Errorf("cannot add %s and %s", v.Kind(), other.Kind())
}

// Sub performs subtraction.
func (v Value) Sub(other Value) (Value, error) {
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			// Return int only if both are actual ints
			if isActualInt(v) && isActualInt(other) {
				return FromInt(int64(f1 - f2)), nil
			}
			return FromFloat(f1 - f2), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot subtract %s from %s", other.Kind(), v.Kind())
}

// Mul performs multiplication.
func (v Value) Mul(other Value) (Value, error) {
	// String repetition
	if s, ok := v.AsString(); ok {
		if n, ok := other.AsInt(); ok && n >= 0 {
			if v.IsSafe() {
				return FromSafeString(strings.Repeat(s, int(n))), nil
			}
			return FromString(strings.Repeat(s, int(n))), nil
		}
	}
	if n, ok := v.AsInt(); ok && n >= 0 {
		if s, ok := other.AsString(); ok {
			if other.IsSafe() {
				return FromSafeString(strings.Repeat(s, int(n))), nil
			}
			return FromString(strings.Repeat(s, int(n))), nil
		}
	}

	// Sequence/Iterator repetition (seq * n)
	if items := v.Iter(); items != nil && v.Kind() != KindString {
		if n, ok := other.AsInt(); ok && n >= 0 {
			result := make([]Value, 0, len(items)*int(n))
			for i := int64(0); i < n; i++ {
				result = append(result, items...)
			}
			return FromSlice(result), nil
		}
	}
	// Sequence/Iterator repetition (n * seq)
	if n, ok := v.AsInt(); ok && n >= 0 {
		if items := other.Iter(); items != nil && other.Kind() != KindString {
			result := make([]Value, 0, len(items)*int(n))
			for i := int64(0); i < n; i++ {
				result = append(result, items...)
			}
			return FromSlice(result), nil
		}
	}

	// Numeric multiplication
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			// Return int only if both are actual ints
			if isActualInt(v) && isActualInt(other) {
				return FromInt(int64(f1 * f2)), nil
			}
			return FromFloat(f1 * f2), nil
		}
	}

	return Undefined(), fmt.Errorf("cannot multiply %s and %s", v.Kind(), other.Kind())
}

// Div performs division.
func (v Value) Div(other Value) (Value, error) {
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			// Float division by zero returns inf/-inf/NaN like Rust
			// Only error on integer division by zero
			if f2 == 0 && isActualInt(v) && isActualInt(other) {
				return Undefined(), fmt.Errorf("division by zero")
			}
			return FromFloat(f1 / f2), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot divide %s by %s", v.Kind(), other.Kind())
}

// FloorDiv performs floor division.
func (v Value) FloorDiv(other Value) (Value, error) {
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			if f2 == 0 {
				return Undefined(), fmt.Errorf("division by zero")
			}
			result := math.Floor(f1 / f2)
			// Return int only if both operands are actual ints
			if isActualInt(v) && isActualInt(other) {
				return FromInt(int64(result)), nil
			}
			return FromFloat(result), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot floor divide %s by %s", v.Kind(), other.Kind())
}

// Rem performs modulo operation.
func (v Value) Rem(other Value) (Value, error) {
	if i1, ok := v.AsInt(); ok {
		if i2, ok := other.AsInt(); ok {
			if i2 == 0 {
				return Undefined(), fmt.Errorf("modulo by zero")
			}
			return FromInt(i1 % i2), nil
		}
	}
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			if f2 == 0 {
				return Undefined(), fmt.Errorf("modulo by zero")
			}
			return FromFloat(math.Mod(f1, f2)), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot modulo %s by %s", v.Kind(), other.Kind())
}

// Pow performs exponentiation.
func (v Value) Pow(other Value) (Value, error) {
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			result := math.Pow(f1, f2)
			// Try to return int if possible
			if _, ok1 := v.AsInt(); ok1 {
				if i2, ok2 := other.AsInt(); ok2 && i2 >= 0 {
					if result == math.Trunc(result) && result <= math.MaxInt64 && result >= math.MinInt64 {
						return FromInt(int64(result)), nil
					}
				}
			}
			return FromFloat(result), nil
		}
	}
	return Undefined(), fmt.Errorf("cannot compute power of %s and %s", v.Kind(), other.Kind())
}

// boolToFloat converts a bool to float for coercion (false=0, true=1)
func boolToFloat(b bool) float64 {
	if b {
		return 1.0
	}
	return 0.0
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
	if f1, ok := v.AsFloat(); ok {
		if f2, ok := other.AsFloat(); ok {
			return f1 == f2
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

	// Sequence comparison
	if seq1, ok := v.AsSlice(); ok {
		if seq2, ok := other.AsSlice(); ok {
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
		return false
	}

	// Map comparison
	if m1, ok := v.AsMap(); ok {
		if m2, ok := other.AsMap(); ok {
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
		return false
	}

	return false
}

// Compare returns -1 if v < other, 0 if equal, 1 if v > other.
// Unlike Equal, Compare uses kind ordering first (like Rust's Ord).
func (v Value) Compare(other Value) (int, bool) {
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

	// Sequence comparison (lexicographic)
	if seq1, ok := v.AsSlice(); ok {
		if seq2, ok := other.AsSlice(); ok {
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

	return 0, false
}

// Contains checks if v contains other.
func (v Value) Contains(other Value) bool {
	switch d := v.data.(type) {
	case string:
		if s, ok := other.AsString(); ok {
			return strings.Contains(d, s)
		}
	case safeString:
		if s, ok := other.AsString(); ok {
			return strings.Contains(string(d), s)
		}
	case []Value:
		for _, item := range d {
			if item.Equal(other) {
				return true
			}
		}
	case map[string]Value:
		if s, ok := other.AsString(); ok {
			_, exists := d[s]
			return exists
		}
	}
	return false
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
