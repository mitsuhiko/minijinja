package value

import (
	"encoding/json"
	"testing"
)

func TestFromAnyKeepsFloats(t *testing.T) {
	tests := []struct {
		name  string
		input any
		want  string
	}{
		{"whole float64", float64(4), "4.0"},
		{"fractional float64", 4.5, "4.5"},
		{"whole float32", float32(4), "4.0"},
		{"int stays an int", 4, "4"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			rv := FromAny(test.input)
			if got := rv.String(); got != test.want {
				t.Fatalf("unexpected rendering: got %s, want %s", got, test.want)
			}
		})
	}

	if !FromAny(float64(4)).IsActualFloat() {
		t.Fatal("a whole float64 must stay a float")
	}
}

func TestFromAnyJSONNumber(t *testing.T) {
	tests := []struct {
		name    string
		input   json.Number
		want    string
		isFloat bool
	}{
		{"integer literal", json.Number("1"), "1", false},
		{"negative integer literal", json.Number("-1"), "-1", false},
		{"float literal", json.Number("1.0"), "1.0", true},
		{"exponent literal", json.Number("1e3"), "1000.0", true},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			rv := FromAny(test.input)
			if got := rv.String(); got != test.want {
				t.Fatalf("unexpected rendering: got %s, want %s", got, test.want)
			}
			if rv.IsActualFloat() != test.isFloat {
				t.Fatalf("unexpected float flag for %s", test.input)
			}
		})
	}
}

func TestFromAnyJSONNumberInsideContainers(t *testing.T) {
	rv := FromAny(map[string]any{"n": json.Number("2"), "f": json.Number("2.5")})
	if got := rv.GetItem(FromString("n")).String(); got != "2" {
		t.Fatalf("unexpected integer in map: %s", got)
	}
	if got := rv.GetItem(FromString("f")).String(); got != "2.5" {
		t.Fatalf("unexpected float in map: %s", got)
	}
}
