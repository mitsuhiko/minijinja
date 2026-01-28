package minijinja

import (
	"errors"
	"fmt"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func renderString(env *Environment, source string, ctx map[string]any) (string, error) {
	tmpl, err := env.TemplateFromString(source)
	if err != nil {
		return "", err
	}
	return tmpl.Render(ctx)
}

func assertRender(t *testing.T, env *Environment, source string, ctx map[string]any, expected string) {
	t.Helper()
	result, err := renderString(env, source, ctx)
	if err != nil {
		t.Fatalf("unexpected render error for %q: %v", source, err)
	}
	if result != expected {
		t.Fatalf("unexpected render result for %q: got %q, want %q", source, result, expected)
	}
}

func assertRenderErrorKind(t *testing.T, env *Environment, source string, ctx map[string]any, expected ErrorKind) {
	t.Helper()
	_, err := renderString(env, source, ctx)
	if err == nil {
		t.Fatalf("expected error for %q", source)
	}
	var mjErr *Error
	if !errors.As(err, &mjErr) {
		t.Fatalf("expected minijinja error for %q, got %T", source, err)
	}
	if mjErr.Kind != expected {
		t.Fatalf("unexpected error kind for %q: got %v, want %v", source, mjErr.Kind, expected)
	}
}

func filterUndefinedBehavior(state FilterState) UndefinedBehavior {
	if provider, ok := state.(interface{ UndefinedBehavior() UndefinedBehavior }); ok {
		return provider.UndefinedBehavior()
	}
	return UndefinedLenient
}

func TestLenientUndefinedBehavior(t *testing.T) {
	env := NewEnvironment()
	env.AddFilter("test", func(state FilterState, val Value, _ []Value, _ map[string]Value) (Value, error) {
		behavior := filterUndefinedBehavior(state)
		if behavior != UndefinedLenient {
			return value.Undefined(), fmt.Errorf("unexpected undefined behavior: %v", behavior)
		}
		if val.String() != "" {
			return value.Undefined(), fmt.Errorf("expected empty string, got %q", val.String())
		}
		return val, nil
	})

	assertRender(t, env, "<{{ true.missing_attribute }}>", nil, "<>")
	assertRenderErrorKind(t, env, "{{ undefined.missing_attribute }}", nil, ErrUndefinedVar)
	assertRender(t, env, "<{% for x in undefined %}...{% endfor %}>", nil, "<>")
	assertRender(t, env, "{{ 'foo' is in(undefined) }}", nil, "false")
	assertRender(t, env, "<{{ undefined }}>", nil, "<>")
	assertRender(t, env, "{{ not undefined }}", nil, "true")
	assertRender(t, env, "{{ undefined is undefined }}", nil, "true")
	assertRender(t, env, "{{ x.foo is undefined }}", map[string]any{"x": map[string]any{}}, "true")
	assertRender(t, env, "{{ undefined|list }}", nil, "[]")
	assertRender(t, env, "<{{ undefined|test }}>", nil, "<>")
	assertRender(t, env, "{{ 42 in undefined }}", nil, "false")
}

func TestSemiStrictUndefinedBehavior(t *testing.T) {
	env := NewEnvironment()
	env.SetUndefinedBehavior(UndefinedSemiStrict)

	assertRenderErrorKind(t, env, "{{ true.missing_attribute }}", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "{{ undefined.missing_attribute }}", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "<{% for x in undefined %}...{% endfor %}>", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "{{ 'foo' is in(undefined) }}", nil, ErrUndefinedVar)
	assertRender(t, env, "<{% if undefined %}42{% endif %}>", nil, "<>")
	assertRenderErrorKind(t, env, "<{{ undefined }}>", nil, ErrUndefinedVar)
	assertRender(t, env, "{{ not undefined }}", nil, "true")
	assertRender(t, env, "{{ undefined is undefined }}", nil, "true")
	assertRender(t, env, "<{{ 42 if false }}>", nil, "<>")
	assertRender(t, env, "{{ x.foo is undefined }}", map[string]any{"x": map[string]any{}}, "true")
	assertRender(t, env, "<{% if x.foo %}...{% endif %}>", map[string]any{"x": map[string]any{}}, "<>")
	assertRenderErrorKind(t, env, "{{ undefined|list }}", nil, ErrInvalidOperation)
	assertRenderErrorKind(t, env, "{{ 42 in undefined }}", nil, ErrUndefinedVar)
}

func TestStrictUndefinedBehavior(t *testing.T) {
	env := NewEnvironment()
	env.SetUndefinedBehavior(UndefinedStrict)

	assertRenderErrorKind(t, env, "{{ true.missing_attribute }}", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "{{ undefined.missing_attribute }}", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "<{% for x in undefined %}...{% endfor %}>", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "{{ 'foo' is in(undefined) }}", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "<{% if undefined %}42{% endif %}>", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "<{{ undefined }}>", nil, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "<{{ not undefined }}>", nil, ErrUndefinedVar)
	assertRender(t, env, "{{ undefined is undefined }}", nil, "true")
	assertRender(t, env, "<{{ 42 if false }}>", nil, "<>")
	assertRender(t, env, "{{ x.foo is undefined }}", map[string]any{"x": map[string]any{}}, "true")
	assertRenderErrorKind(t, env, "{% if x.foo %}...{% endif %}", map[string]any{"x": map[string]any{}}, ErrUndefinedVar)
	assertRenderErrorKind(t, env, "{{ undefined|list }}", nil, ErrInvalidOperation)
	assertRenderErrorKind(t, env, "{{ 42 in undefined }}", nil, ErrUndefinedVar)
}

func TestChainableUndefinedBehavior(t *testing.T) {
	env := NewEnvironment()
	env.SetUndefinedBehavior(UndefinedChainable)
	env.AddFilter("test", func(state FilterState, val Value, _ []Value, _ map[string]Value) (Value, error) {
		behavior := filterUndefinedBehavior(state)
		if behavior != UndefinedChainable {
			return value.Undefined(), fmt.Errorf("unexpected undefined behavior: %v", behavior)
		}
		if val.String() != "" {
			return value.Undefined(), fmt.Errorf("expected empty string, got %q", val.String())
		}
		return val, nil
	})

	assertRender(t, env, "<{{ true.missing_attribute }}>", nil, "<>")
	assertRender(t, env, "<{{ undefined.missing_attribute }}>", nil, "<>")
	assertRender(t, env, "<{% for x in undefined %}...{% endfor %}>", nil, "<>")
	assertRender(t, env, "{{ x.foo is undefined }}", map[string]any{"x": map[string]any{}}, "true")
	assertRender(t, env, "{{ 'foo' is in(undefined) }}", nil, "false")
	assertRender(t, env, "<{{ undefined }}>", nil, "<>")
	assertRender(t, env, "{{ not undefined }}", nil, "true")
	assertRender(t, env, "{{ undefined is undefined }}", nil, "true")
	assertRender(t, env, "{{ undefined|list }}", nil, "[]")
	assertRender(t, env, "<{{ undefined|test }}>", nil, "<>")
	assertRender(t, env, "{{ 42 in undefined }}", nil, "false")
}
