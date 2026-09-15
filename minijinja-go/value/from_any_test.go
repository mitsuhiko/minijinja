package value

import (
	"encoding/json"
	"testing"
)

type namedFloat float64

func TestFromAnyPreservesFloatTypes(t *testing.T) {
	tests := []struct {
		name  string
		input any
		want  string
	}{
		{"float64", float64(4), "4.0"},
		{"float32", float32(4), "4.0"},
		{"named float", namedFloat(4), "4.0"},
		{"fractional float", 4.5, "4.5"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := FromAny(test.input)
			if !value.IsActualFloat() {
				t.Fatal("value was not stored as a float")
			}
			if got := value.String(); got != test.want {
				t.Fatalf("unexpected value: got %s, want %s", got, test.want)
			}
		})
	}
}

func TestFromAnyConvertsJSONNumberByLiteral(t *testing.T) {
	tests := []struct {
		literal string
		want    string
		kind    ValueKind
		isFloat bool
	}{
		{"1", "1", KindNumber, false},
		{"-1", "-1", KindNumber, false},
		{"1.0", "1.0", KindNumber, true},
		{"1e3", "1000.0", KindNumber, true},
		{"1e400", "", KindUndefined, false},
		{"01", "", KindUndefined, false},
		{"NaN", "", KindUndefined, false},
		{"invalid", "", KindUndefined, false},
	}

	for _, test := range tests {
		t.Run(test.literal, func(t *testing.T) {
			value := FromAny(json.Number(test.literal))
			if value.Kind() != test.kind {
				t.Fatalf("unexpected kind for %q: got %v, want %v", test.literal, value.Kind(), test.kind)
			}
			if value.IsActualFloat() != test.isFloat {
				t.Fatalf("unexpected float classification for %q", test.literal)
			}
			if got := value.String(); got != test.want {
				t.Fatalf("unexpected value: got %s, want %s", got, test.want)
			}
		})
	}
}

func TestFromAnyConvertsJSONNumbersInContainers(t *testing.T) {
	value := FromAny(map[string]json.Number{
		"integer": "2",
		"float":   "2.5",
	})
	if got := value.GetItem(FromString("integer")); got.String() != "2" || !got.IsActualInt() {
		t.Fatalf("unexpected integer value %v", got)
	}
	if got := value.GetItem(FromString("float")); got.String() != "2.5" || !got.IsActualFloat() {
		t.Fatalf("unexpected float value %v", got)
	}
}
