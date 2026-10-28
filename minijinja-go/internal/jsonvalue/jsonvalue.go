// Package jsonvalue converts template values into Go values for JSON
// serialization, matching the output of the Rust implementation.
package jsonvalue

import (
	"math"
	"strconv"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// ToNative converts a value into a Go value that encoding/json serializes
// the same way the Rust implementation (serde_json) does.
func ToNative(v value.Value) interface{} {
	switch v.Kind() {
	case value.KindUndefined, value.KindNone:
		return nil
	case value.KindBool:
		b, _ := v.AsBool()
		return b
	case value.KindNumber:
		if i, ok := v.AsInt(); ok && v.IsActualInt() {
			return i
		}
		f, _ := v.AsFloat()
		return jsonFloat(f)
	case value.KindString:
		s, _ := v.AsString()
		return s
	case value.KindSeq, value.KindIterable:
		items := v.Iter()
		result := make([]interface{}, len(items))
		for i, item := range items {
			result[i] = ToNative(item)
		}
		return result
	case value.KindMap:
		m, _ := v.AsMap()
		result := make(map[string]interface{})
		for k, val := range m {
			result[k] = ToNative(val)
		}
		return result
	default:
		if m, ok := v.AsMap(); ok {
			result := make(map[string]interface{})
			for k, val := range m {
				result[k] = ToNative(val)
			}
			return result
		}
		return v.String()
	}
}

// jsonFloat serializes floats like the Rust implementation (serde_json): a
// float stays a float in the output (`1.0`, not `1`), large and small
// magnitudes use exponent notation, and non-finite values become `null`.
type jsonFloat float64

func (f jsonFloat) MarshalJSON() ([]byte, error) {
	v := float64(f)
	if math.IsNaN(v) || math.IsInf(v, 0) {
		return []byte("null"), nil
	}
	if v == 0 {
		if math.Signbit(v) {
			return []byte("-0.0"), nil
		}
		return []byte("0.0"), nil
	}

	// Shortest representation, e.g. "-1.2345e+06"
	formatted := strconv.FormatFloat(v, 'e', -1, 64)
	sign := ""
	if formatted[0] == '-' {
		sign = "-"
		formatted = formatted[1:]
	}
	mantissa, expPart, _ := strings.Cut(formatted, "e")
	exp, err := strconv.Atoi(expPart)
	if err != nil {
		return nil, err
	}
	digits := strings.Replace(mantissa, ".", "", 1)
	// Number of digits before the decimal point
	pointPos := exp + 1

	var out string
	switch {
	case pointPos > 0 && pointPos <= 16:
		if pointPos >= len(digits) {
			out = digits + strings.Repeat("0", pointPos-len(digits)) + ".0"
		} else {
			out = digits[:pointPos] + "." + digits[pointPos:]
		}
	case pointPos > -5 && pointPos <= 0:
		out = "0." + strings.Repeat("0", -pointPos) + digits
	default:
		out = digits[:1]
		if len(digits) > 1 {
			out += "." + digits[1:]
		}
		if exp > 0 {
			out += "e+" + strconv.Itoa(exp)
		} else {
			out += "e" + strconv.Itoa(exp)
		}
	}
	return []byte(sign + out), nil
}
