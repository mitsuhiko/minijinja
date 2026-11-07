// Package jsonvalue converts template values into Go values for JSON
// serialization, matching the output of the Rust implementation.
package jsonvalue

import (
	"bytes"
	"encoding/json"
	"errors"
	"math"
	"strconv"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// ErrUnsupportedKey is returned for map keys that cannot be JSON object
// keys.  Like serde_json in Rust, strings, numbers and booleans are
// supported keys.
var ErrUnsupportedKey = errors.New("cannot serialize to JSON")

// ToNative converts a value into a Go value that encoding/json serializes
// the same way the Rust implementation (serde_json) does.  Maps keep their
// key order.
func ToNative(v value.Value) (interface{}, error) {
	switch v.Kind() {
	case value.KindUndefined, value.KindNone:
		return nil, nil
	case value.KindBool:
		b, _ := v.AsBool()
		return b, nil
	case value.KindNumber:
		if i, ok := v.AsInt(); ok && v.IsActualInt() {
			return i, nil
		}
		if f, ok := v.AsFloat(); ok {
			return jsonFloat(f), nil
		}
		// An integer outside the 64-bit range, which serde_json writes
		// out in full like any other integer
		return json.Number(v.String()), nil
	case value.KindString:
		s, _ := v.AsString()
		return s, nil
	case value.KindSeq, value.KindIterable:
		items := v.Iter()
		result := make([]interface{}, len(items))
		for i, item := range items {
			native, err := ToNative(item)
			if err != nil {
				return nil, err
			}
			result[i] = native
		}
		return result, nil
	}
	m, ok := v.AsValueMap()
	if !ok && v.Kind() == value.KindMap {
		// map objects
		m = value.NewMap()
		for _, k := range v.Iter() {
			m.Set(k, v.GetItem(k))
		}
		ok = true
	}
	if ok {
		result := &object{
			keys:   make([]string, 0, m.Len()),
			values: make([]interface{}, 0, m.Len()),
		}
		for k, val := range m.All() {
			key, err := objectKey(k)
			if err != nil {
				return nil, err
			}
			native, err := ToNative(val)
			if err != nil {
				return nil, err
			}
			result.keys = append(result.keys, key)
			result.values = append(result.values, native)
		}
		return result, nil
	}
	return v.String(), nil
}

// objectKey converts a map key into a JSON object key like serde_json.
func objectKey(k value.Value) (string, error) {
	switch k.Kind() {
	case value.KindString:
		s, _ := k.AsString()
		return s, nil
	case value.KindBool:
		if b, _ := k.AsBool(); b {
			return "true", nil
		}
		return "false", nil
	case value.KindNumber:
		if k.IsActualFloat() {
			f, _ := k.AsFloat()
			if math.IsNaN(f) || math.IsInf(f, 0) {
				return "", ErrUnsupportedKey
			}
			b, err := jsonFloat(f).MarshalJSON()
			return string(b), err
		}
		return k.String(), nil
	}
	return "", ErrUnsupportedKey
}

// object is a JSON object that keeps the order of its keys.
type object struct {
	keys   []string
	values []interface{}
}

func (o *object) MarshalJSON() ([]byte, error) {
	var buf bytes.Buffer
	// HTML characters are escaped (or not) by the encoder that serializes
	// the whole value.
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	buf.WriteByte('{')
	for i, key := range o.keys {
		if i > 0 {
			buf.WriteByte(',')
		}
		if err := enc.Encode(key); err != nil {
			return nil, err
		}
		buf.Truncate(buf.Len() - 1) // strip the newline Encode adds
		buf.WriteByte(':')
		if err := enc.Encode(o.values[i]); err != nil {
			return nil, err
		}
		buf.Truncate(buf.Len() - 1)
	}
	buf.WriteByte('}')
	return buf.Bytes(), nil
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
