package minijinja

import (
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func TestLoopChanged(t *testing.T) {
	tests := []struct {
		name    string
		source  string
		context map[string]any
		want    string
	}{
		{
			name:    "repeated values",
			source:  "{% for x in items %}{{ loop.changed(x) }} {% endfor %}",
			context: map[string]any{"items": []any{1, 1, 2, 2, 1}},
			want:    "True False True False True ",
		},
		{
			name:    "several arguments",
			source:  "{% for x in items %}{{ loop.changed(x, 'k') }} {% endfor %}",
			context: map[string]any{"items": []any{"a", "a", "b"}},
			want:    "True False True ",
		},
		{
			name: "grouping headers",
			source: "{% for row in rows %}{% if loop.changed(row.group) %}[{{ row.group }}]{% endif %}" +
				"{{ row.name }}{% endfor %}",
			context: map[string]any{"rows": []any{
				map[string]any{"group": "a", "name": "one"},
				map[string]any{"group": "a", "name": "two"},
				map[string]any{"group": "b", "name": "three"},
			}},
			want: "[a]onetwo[b]three",
		},
		{
			name:    "nested loops keep their own memory",
			source:  "{% for x in outer %}{% for y in inner %}{{ loop.changed(y) }} {% endfor %}{% endfor %}",
			context: map[string]any{"outer": []any{1, 2}, "inner": []any{1, 1}},
			want:    "True False True False ",
		},
		{
			name:    "iterator without a known length",
			source:  "{% for x in it %}{{ loop.changed(x) }} {% endfor %}",
			context: map[string]any{"it": makeChangedIterator()},
			want:    "True False True ",
		},
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

func makeChangedIterator() value.Value {
	return value.MakeOneShotIterator(func(yield func(value.Value) bool) {
		for _, item := range []value.Value{value.FromInt(1), value.FromInt(1), value.FromInt(2)} {
			if !yield(item) {
				return
			}
		}
	})
}
