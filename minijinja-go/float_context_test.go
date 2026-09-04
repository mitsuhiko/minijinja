package minijinja

import (
	"encoding/json"
	"strings"
	"testing"
)

// TestFloatFromContext pins how numbers coming from the Go context render:
// a float stays a float, the way Jinja2 renders a Python float, and a
// json.Number keeps whatever its literal said.
func TestFloatFromContext(t *testing.T) {
	tests := []struct {
		name    string
		source  string
		context map[string]any
		want    string
	}{
		{"whole float", "{{ x }}", map[string]any{"x": float64(4)}, "4.0"},
		{"fractional float", "{{ x }}", map[string]any{"x": 4.5}, "4.5"},
		{"float arithmetic", "{{ x + 1 }}", map[string]any{"x": float64(4)}, "5.0"},
		{"float test", "{{ x is float }}", map[string]any{"x": float64(4)}, "True"},
		{"int stays an int", "{{ x }}", map[string]any{"x": 4}, "4"},
		{"json integer literal", "{{ x }}", decodeJSON(t, `{"x": 1}`), "1"},
		{"json float literal", "{{ x }}", decodeJSON(t, `{"x": 1.0}`), "1.0"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			env := NewEnvironment()
			if err := env.AddTemplate("t", test.source); err != nil {
				t.Fatalf("add template: %v", err)
			}
			tmpl, err := env.GetTemplate("t")
			if err != nil {
				t.Fatalf("get template: %v", err)
			}
			got, err := tmpl.Render(test.context)
			if err != nil {
				t.Fatalf("render: %v", err)
			}
			if got != test.want {
				t.Fatalf("unexpected output: got %q, want %q", got, test.want)
			}
		})
	}
}

func decodeJSON(t *testing.T, source string) map[string]any {
	t.Helper()
	decoder := json.NewDecoder(strings.NewReader(source))
	decoder.UseNumber()
	var out map[string]any
	if err := decoder.Decode(&out); err != nil {
		t.Fatalf("decode context: %v", err)
	}
	return out
}
