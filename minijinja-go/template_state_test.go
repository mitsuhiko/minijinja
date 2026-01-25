package minijinja

import (
	"bytes"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func TestEvalToState(t *testing.T) {
	env := NewEnvironment()

	err := env.AddTemplate("test.html", `
{% macro greet(name) %}Hello {{ name }}!{% endmacro %}
{% block title %}Default Title{% endblock %}
{% set version = "1.0" %}
`)
	if err != nil {
		t.Fatal(err)
	}

	tmpl, err := env.GetTemplate("test.html")
	if err != nil {
		t.Fatal(err)
	}

	state, err := tmpl.EvalToState(map[string]any{
		"user": "John",
	})
	if err != nil {
		t.Fatal(err)
	}

	// Test Name (State returns the template name)
	if state.Name() != "test.html" {
		t.Errorf("expected name 'test.html', got %q", state.Name())
	}

	// Test RenderBlock
	title, err := state.RenderBlock("title")
	if err != nil {
		t.Fatal(err)
	}
	if title != "Default Title" {
		t.Errorf("expected 'Default Title', got %q", title)
	}

	// Test CallMacro
	result, err := state.CallMacro("greet", value.FromString("World"))
	if err != nil {
		t.Fatal(err)
	}
	if result.String() != "Hello World!" {
		t.Errorf("expected 'Hello World!', got %q", result.String())
	}

	// Test Lookup
	ver := state.Lookup("version")
	if v, ok := ver.AsString(); !ok || v != "1.0" {
		t.Errorf("expected version '1.0', got %v", ver)
	}

	user := state.Lookup("user")
	if v, ok := user.AsString(); !ok || v != "John" {
		t.Errorf("expected user 'John', got %v", user)
	}

	// Test Exports
	exports := state.Exports()
	if _, ok := exports["version"]; !ok {
		t.Error("expected 'version' in exports")
	}
	if _, ok := exports["greet"]; !ok {
		t.Error("expected 'greet' macro in exports")
	}

	// Test BlockNames
	blocks := state.BlockNames()
	found := false
	for _, b := range blocks {
		if b == "title" {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected 'title' in block names, got %v", blocks)
	}

	// Test MacroNames
	macros := state.MacroNames()
	found = false
	for _, m := range macros {
		if m == "greet" {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected 'greet' in macro names, got %v", macros)
	}
}

func TestEvalToStateInheritance(t *testing.T) {
	env := NewEnvironment()

	err := env.AddTemplate("base.html", `
<!doctype html>
<title>{% block title %}Base{% endblock %}</title>
<body>{% block body %}{% endblock %}</body>
`)
	if err != nil {
		t.Fatal(err)
	}

	err = env.AddTemplate("child.html", `
{% extends "base.html" %}
{% block title %}Child Page{% endblock %}
{% block body %}Content here{% endblock %}
`)
	if err != nil {
		t.Fatal(err)
	}

	tmpl, err := env.GetTemplate("child.html")
	if err != nil {
		t.Fatal(err)
	}

	state, err := tmpl.EvalToState(nil)
	if err != nil {
		t.Fatal(err)
	}

	title, err := state.RenderBlock("title")
	if err != nil {
		t.Fatal(err)
	}
	if title != "Child Page" {
		t.Errorf("expected 'Child Page', got %q", title)
	}

	body, err := state.RenderBlock("body")
	if err != nil {
		t.Fatal(err)
	}
	if body != "Content here" {
		t.Errorf("expected 'Content here', got %q", body)
	}
}

func TestRenderToWrite(t *testing.T) {
	env := NewEnvironment()

	tmpl, err := env.TemplateFromString("Hello {{ name }}!")
	if err != nil {
		t.Fatal(err)
	}

	var buf bytes.Buffer
	err = tmpl.RenderToWrite(map[string]any{"name": "World"}, &buf)
	if err != nil {
		t.Fatal(err)
	}

	if buf.String() != "Hello World!" {
		t.Errorf("expected 'Hello World!', got %q", buf.String())
	}
}

func TestSetFormatter(t *testing.T) {
	env := NewEnvironment()

	// Set formatter that treats None as empty
	env.SetFormatter(func(state *State, val value.Value, escape func(string) string) string {
		if val.IsNone() {
			return ""
		}
		s := val.String()
		if !val.IsSafe() {
			s = escape(s)
		}
		return s
	})

	tmpl, err := env.TemplateFromString("Value: [{{ val }}]")
	if err != nil {
		t.Fatal(err)
	}

	// Test with None
	result, err := tmpl.Render(map[string]any{"val": nil})
	if err != nil {
		t.Fatal(err)
	}
	if result != "Value: []" {
		t.Errorf("expected 'Value: []', got %q", result)
	}

	// Test with actual value
	result, err = tmpl.Render(map[string]any{"val": "hello"})
	if err != nil {
		t.Fatal(err)
	}
	if result != "Value: [hello]" {
		t.Errorf("expected 'Value: [hello]', got %q", result)
	}
}

func TestCallMacroKw(t *testing.T) {
	env := NewEnvironment()

	err := env.AddTemplate("test.html", `
{% macro input(name, value="", type="text") -%}
<input name="{{ name }}" value="{{ value }}" type="{{ type }}">
{%- endmacro %}
`)
	if err != nil {
		t.Fatal(err)
	}

	tmpl, err := env.GetTemplate("test.html")
	if err != nil {
		t.Fatal(err)
	}

	state, err := tmpl.EvalToState(nil)
	if err != nil {
		t.Fatal(err)
	}

	// Call with kwargs
	result, err := state.CallMacroKw("input",
		[]value.Value{value.FromString("email")},
		map[string]value.Value{"type": value.FromString("email")},
	)
	if err != nil {
		t.Fatal(err)
	}

	expected := `<input name="email" value="" type="email">`
	if result.String() != expected {
		t.Errorf("expected %q, got %q", expected, result.String())
	}
}
