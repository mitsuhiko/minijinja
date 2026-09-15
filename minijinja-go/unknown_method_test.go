package minijinja

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

type staticCallable string

func (c staticCallable) Call(value.State, []value.Value, map[string]value.Value) (value.Value, error) {
	return value.FromString(string(c)), nil
}

type firstArgCallable struct{}

func (firstArgCallable) Call(_ value.State, args []value.Value, _ map[string]value.Value) (value.Value, error) {
	return args[0], nil
}

type methodObject struct {
	attrs map[string]value.Value
}

func (o *methodObject) GetAttr(name string) value.Value {
	if attr, ok := o.attrs[name]; ok {
		return attr
	}
	return value.Undefined()
}

func (o *methodObject) CallMethod(_ value.State, name string, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
	if name == "resolve" {
		return value.FromString("method"), nil
	}
	return value.Undefined(), fmt.Errorf("method lookup: %w", value.ErrUnknownMethod)
}

func TestUnknownMethodCallback(t *testing.T) {
	env := NewEnvironment()
	env.SetUnknownMethodCallback(func(_ *State, val value.Value, method string, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		if method != "inspect" {
			return value.Undefined(), value.ErrUnknownMethod
		}
		receiver, _ := val.AsString()
		return value.FromString(fmt.Sprintf("%s:%s:%s", receiver, args[0], kwargs["suffix"])), nil
	})

	tmpl, err := env.TemplateFromString(`{{ "value".inspect("arg", suffix="kwarg") }}`)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatal(err)
	}
	if result != "value:arg:kwarg" {
		t.Fatalf("unexpected result %q", result)
	}
}

func TestUnknownMethodResolutionOrder(t *testing.T) {
	env := NewEnvironment()
	env.SetUnknownMethodCallback(func(_ *State, _ value.Value, method string, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		if method == "resolve" {
			return value.FromString("callback"), nil
		}
		return value.Undefined(), value.ErrUnknownMethod
	})

	context := map[string]value.Value{
		"object": value.FromObject(&methodObject{attrs: map[string]value.Value{
			"resolve": value.FromCallable(staticCallable("attribute")),
		}}),
		"mapping": value.FromMap(map[string]value.Value{
			"resolve": value.FromCallable(staticCallable("attribute")),
		}),
		"fallback": value.FromObject(&methodObject{attrs: map[string]value.Value{
			"fallback": value.FromCallable(staticCallable("attribute")),
		}}),
	}

	tmpl, err := env.TemplateFromString(`{{ object.resolve() }}|{{ mapping.resolve() }}|{{ fallback.fallback() }}`)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(context)
	if err != nil {
		t.Fatal(err)
	}
	if result != "method|callback|attribute" {
		t.Fatalf("unexpected resolution order %q", result)
	}
}

func TestUnknownMethodArgumentsEvaluatedOnce(t *testing.T) {
	env := NewEnvironment()
	calls := 0
	env.AddFunction("bump", func(*State, []value.Value, map[string]value.Value) (value.Value, error) {
		calls++
		return value.FromInt(int64(calls)), nil
	})
	env.SetUnknownMethodCallback(func(*State, value.Value, string, []value.Value, map[string]value.Value) (value.Value, error) {
		return value.Undefined(), fmt.Errorf("callback lookup: %w", value.ErrUnknownMethod)
	})

	obj := &methodObject{attrs: map[string]value.Value{
		"fallback": value.FromCallable(firstArgCallable{}),
	}}
	tmpl, err := env.TemplateFromString(`{{ obj.fallback(bump()) }}`)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(map[string]value.Value{"obj": value.FromObject(obj)})
	if err != nil {
		t.Fatal(err)
	}
	if result != "1" || calls != 1 {
		t.Fatalf("got result %q after %d argument evaluations", result, calls)
	}
}

func TestUnknownMethodCallbackCanHandleUndefined(t *testing.T) {
	env := NewEnvironment()
	env.SetUndefinedBehavior(UndefinedStrict)
	env.SetUnknownMethodCallback(func(_ *State, val value.Value, _ string, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		if val.IsUndefined() {
			return value.FromString("handled"), nil
		}
		return value.Undefined(), value.ErrUnknownMethod
	})

	tmpl, err := env.TemplateFromString(`{{ missing.method() }}`)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatal(err)
	}
	if result != "handled" {
		t.Fatalf("unexpected result %q", result)
	}
}

func TestUnknownMethodCallbackErrors(t *testing.T) {
	t.Run("unresolved", func(t *testing.T) {
		env := NewEnvironment()
		env.SetUnknownMethodCallback(func(*State, value.Value, string, []value.Value, map[string]value.Value) (value.Value, error) {
			return value.Undefined(), value.ErrUnknownMethod
		})
		tmpl, err := env.TemplateFromString(`{{ "value".missing() }}`)
		if err != nil {
			t.Fatal(err)
		}
		_, err = tmpl.Render(nil)
		var templateErr *Error
		if !errors.As(err, &templateErr) {
			t.Fatalf("expected template error, got %v", err)
		}
		if templateErr.Kind != ErrUnknownMethod {
			t.Fatalf("expected unknown method error, got %v", templateErr.Kind)
		}
	})

	t.Run("callback error", func(t *testing.T) {
		env := NewEnvironment()
		callbackErr := errors.New("callback failed")
		env.SetUnknownMethodCallback(func(*State, value.Value, string, []value.Value, map[string]value.Value) (value.Value, error) {
			return value.Undefined(), callbackErr
		})
		tmpl, err := env.TemplateFromString(`{{ "value".missing() }}`)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := tmpl.Render(nil); !errors.Is(err, callbackErr) {
			t.Fatalf("expected callback error, got %v", err)
		}
	})
}

func TestUnknownMethodPythonCompatibility(t *testing.T) {
	env := NewEnvironment()
	env.SetUnknownMethodCallback(func(_ *State, val value.Value, method string, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		if method == "upper" {
			str, ok := val.AsString()
			if ok {
				return value.FromString(strings.ToUpper(str)), nil
			}
		}
		return value.Undefined(), value.ErrUnknownMethod
	})

	tmpl, err := env.TemplateFromString(`{{ "hello".upper() }}`)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatal(err)
	}
	if result != "HELLO" {
		t.Fatalf("unexpected result %q", result)
	}
}
