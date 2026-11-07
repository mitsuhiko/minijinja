package minijinja

import (
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// TestMapLiteralKeysAndOrder checks that maps created by templates keep the
// type and the insertion order of their keys like MiniJinja for Rust (with
// preserve_order, which all its bindings use) and Jinja2.
func TestMapLiteralKeysAndOrder(t *testing.T) {
	env := NewEnvironment()
	ctx := map[string]any{
		"intmap": map[int]string{2: "b", 1: "a"},
		"strmap": map[string]int{"b": 1, "a": 2, "1": 3},
		"merged": MergeMaps(map[string]any{"a": 1}, map[string]any{"b": 2, "a": 3}),
	}
	tests := []struct {
		template string
		expected string
	}{
		{`{{ {"b": 1, "a": 2} }}`, `{'b': 1, 'a': 2}`},
		{`{{ {"b": 1, "a": 2}|list }}`, `['b', 'a']`},
		{`{{ {"b": 1, "a": 2}|items }}`, `[('b', 1), ('a', 2)]`},
		{`{{ {"b": 1, "a": 2}|first }}|{{ {"b": 1, "a": 2}|last }}`, `b|a`},
		{`{{ {"b": 1, "a": 2}|reverse|list }}`, `['a', 'b']`},
		{`{{ {"b": 1, "a": 2}|dictsort }}`, `[('a', 2), ('b', 1)]`},
		{`{{ {"b": 1, "a": 2}|tojson }}`, `{"b": 1, "a": 2}`},
		{`{{ {"b": 1, "a": {"y": 1, "x": "<"}}|tojson }}`, `{"b": 1, "a": {"y": 1, "x": "\u003c"}}`},
		{`{{ {1: "a", true: "b", 1.5: "c"}|tojson }}`, `{"1": "b", "1.5": "c"}`},
		{`{{ {false: 1}|tojson }}`, `{"false": 1}`},
		{`{{ {"b": 1, "a": 2}|urlencode }}`, `b=1&a=2`},
		{`{{ {"b": 1, "a": 2}|pprint }}`, "{\n    'b': 1,\n    'a': 2,\n}"},
		{`{% for k in {"b": 1, "a": 2} %}{{ loop.index }}{{ k }}{% endfor %}`, `1b2a`},
		{`{{ dict({"b": 1, "a": 2}, c=3) }}`, `{'b': 1, 'a': 2, 'c': 3}`},
		{`{{ dict([(2, "x"), ("a", 3)]) }}`, `{2: 'x', 'a': 3}`},
		{`{{ {"b": 1, 2: 2}|chain({"a": 3, 1: 4}) }}`, `{1: 4, 2: 2, 'a': 3, 'b': 1}`},
		{`{{ "%(a)s"|format({"a": 1}) }}`, `1`},
		{`{{ {10: 1} < {9: 1} }}|{{ [{10: 1}, {9: 1}, {"a": 1}]|sort }}`, `False|[{9: 1}, {10: 1}, {'a': 1}]`},
		// Go maps with keys that are not strings keep their keys as values
		{`{{ intmap }}|{{ intmap[1] }}|{{ 2 in intmap }}|{{ "2" in intmap }}`, `{1: 'a', 2: 'b'}|a|True|False`},
		// Go maps with string keys iterate in sorted order and their keys
		// are strings, even if they look like numbers
		{`{{ strmap }}|{{ strmap["1"] }}|{{ strmap[1] is undefined }}`, `{'1': 3, 'a': 2, 'b': 1}|3|True`},
		{`{{ strmap == {"b": 1, "1": 3, "a": 2} }}|{{ strmap == {"b": 1, 1: 3, "a": 2} }}`, `True|False`},
		// merged maps render like maps
		{`{{ [merged] }}|{{ merged }}|{{ merged|tojson }}`, `[{'a': 3, 'b': 2}]|{'a': 3, 'b': 2}|{"a": 3, "b": 2}`},
	}
	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(ctx)
		if err != nil {
			t.Errorf("render error for %q: %v", test.template, err)
			continue
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestMapKeysTojsonErrors(t *testing.T) {
	env := NewEnvironment()
	for _, source := range []string{
		`{{ {none: 1}|tojson }}`,
		`{{ {(1, 2): 1}|tojson }}`,
		`{{ {"a": {[1]: 1}}|tojson }}`,
	} {
		tmpl, err := env.TemplateFromString(source)
		if err != nil {
			t.Fatalf("parse error for %q: %v", source, err)
		}
		_, err = tmpl.Render(nil)
		if err == nil || !strings.Contains(err.Error(), "cannot serialize to JSON") {
			t.Errorf("%q: expected a serialization error, got %v", source, err)
		}
	}
}

func TestMapKeysFromGo(t *testing.T) {
	m := value.NewMap()
	m.Set(value.FromInt(1), value.FromString("one"))
	m.Set(value.FromString("two"), value.FromInt(2))
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{{ m }}|{{ m[1] }}|{{ m.two }}|{{ m|items }}`)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(map[string]any{"m": value.FromValueMap(m)})
	if err != nil {
		t.Fatal(err)
	}
	if expected := `{1: 'one', 'two': 2}|one|2|[(1, 'one'), ('two', 2)]`; result != expected {
		t.Errorf("expected %q, got %q", expected, result)
	}

	// maps created by templates are value maps and can be read from Go
	env.AddFunction("inspect", func(_ *State, args []Value, _ map[string]Value) (Value, error) {
		vm, ok := args[0].AsValueMap()
		if !ok {
			return value.FromString("not a map"), nil
		}
		parts := []string{}
		for k, v := range vm.All() {
			parts = append(parts, k.Repr()+"="+v.Repr())
		}
		sm, _ := args[0].AsMap()
		return value.FromString(strings.Join(parts, ",") + "|" + sm["1"].String()), nil
	})
	tmpl, err = env.TemplateFromString(`{{ inspect({"b": 1, 1: "x"}) }}`)
	if err != nil {
		t.Fatal(err)
	}
	result, err = tmpl.Render(nil)
	if err != nil {
		t.Fatal(err)
	}
	if expected := `'b'=1,1='x'|x`; result != expected {
		t.Errorf("expected %q, got %q", expected, result)
	}
}
