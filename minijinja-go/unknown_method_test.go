package minijinja

import (
	"errors"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// pyCompatCallback implements the two Python methods used by the tests below.
func pyCompatCallback(_ *State, val value.Value, method string, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	str, ok := val.AsString()
	if !ok {
		return value.Undefined(), value.ErrUnknownMethod
	}
	switch method {
	case "upper":
		return value.FromString(strings.ToUpper(str)), nil
	case "split":
		sep := " "
		if len(args) > 0 {
			if s, ok := args[0].AsString(); ok {
				sep = s
			}
		}
		parts := strings.Split(str, sep)
		items := make([]value.Value, len(parts))
		for i, part := range parts {
			items[i] = value.FromString(part)
		}
		return value.FromSlice(items), nil
	}
	return value.Undefined(), value.ErrUnknownMethod
}

func TestUnknownMethodCallback(t *testing.T) {
	env := NewEnvironment()
	env.SetUnknownMethodCallback(pyCompatCallback)

	tests := []struct {
		template string
		expected string
	}{
		{`{{ "hello".upper() }}`, "HELLO"},
		{`{{ "a-b-c".split("-")[1] }}`, "b"},
		{`{{ "a b".split()|length }}`, "2"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(nil)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

// TestUnknownMethodCallbackUnresolved checks that value.ErrUnknownMethod leaves
// the call unresolved, and that any other error is reported to the caller.
func TestUnknownMethodCallbackUnresolved(t *testing.T) {
	env := NewEnvironment()
	env.SetUnknownMethodCallback(pyCompatCallback)

	tmpl, err := env.TemplateFromString(`{{ "hello".nope() }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	if _, err := tmpl.Render(nil); err == nil {
		t.Error("expected an error for an unresolved method")
	}

	failing := errors.New("method failed")
	env = NewEnvironment()
	env.SetUnknownMethodCallback(func(*State, value.Value, string, []value.Value, map[string]value.Value) (value.Value, error) {
		return value.Undefined(), failing
	})
	tmpl, err = env.TemplateFromString(`{{ "hello".nope() }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	if _, err := tmpl.Render(nil); err == nil || !strings.Contains(err.Error(), failing.Error()) {
		t.Errorf("expected the callback error to be reported, got %v", err)
	}
}

// keyedMap exposes a callable stored under a name that the callback also handles.
type keyedMap struct{}

func (keyedMap) Call(_ value.State, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	return value.FromString("item"), nil
}

// TestUnknownMethodCallbackPrecedence checks that the callback wins over a
// same-named item on the value, the way type methods do in Jinja2.
func TestUnknownMethodCallbackPrecedence(t *testing.T) {
	env := NewEnvironment()
	env.SetUnknownMethodCallback(func(_ *State, _ value.Value, method string, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		if method == "upper" {
			return value.FromString("method"), nil
		}
		return value.Undefined(), value.ErrUnknownMethod
	})

	tmpl, err := env.TemplateFromString(`{{ obj.upper() }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	result, err := tmpl.Render(map[string]value.Value{
		"obj": value.FromMap(map[string]value.Value{"upper": value.FromCallable(keyedMap{})}),
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "method" {
		t.Errorf("expected the callback to take precedence, got %q", result)
	}
}

// TestUnknownMethodCallbackArguments checks that positional and keyword
// arguments reach the callback.
func TestUnknownMethodCallbackArguments(t *testing.T) {
	env := NewEnvironment()
	env.SetUnknownMethodCallback(func(_ *State, _ value.Value, _ string, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		parts := make([]string, 0, len(args)+len(kwargs))
		for _, arg := range args {
			parts = append(parts, arg.String())
		}
		if sep, ok := kwargs["sep"]; ok {
			parts = append(parts, "sep="+sep.String())
		}
		return value.FromString(strings.Join(parts, ",")), nil
	})

	tmpl, err := env.TemplateFromString(`{{ "x".anything(1, 2, sep="-") }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "1,2,sep=-" {
		t.Errorf("expected arguments to reach the callback, got %q", result)
	}
}
