package minijinja

import (
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func TestFormatFilter(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{{ "%s, %s!"|format(greeting, name) }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"greeting": "Hello",
		"name":     "World",
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "Hello, World!" {
		t.Errorf("expected 'Hello, World!', got %q", result)
	}

	tmplMap, err := env.TemplateFromString(`{{ "%(greet)s, %(name)s!"|format(data) }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err = tmplMap.Render(map[string]any{
		"data": map[string]any{
			"greet": "Hello",
			"name":  "World",
		},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "Hello, World!" {
		t.Errorf("expected 'Hello, World!', got %q", result)
	}
}

func TestOperatorAliases(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{{ [1,2,3]|select("==", 2)|join(",") }}|{{ [1,2,3]|select("!=", 2)|join(",") }}|{{ [1,2,3]|select("lessthan", 3)|join(",") }}|{{ [1,2,3]|select("greaterthan", 1)|join(",") }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	expected := "2|1,3|1,2|2,3"
	if result != expected {
		t.Errorf("expected %q, got %q", expected, result)
	}
}

func TestTemplateManagementAPIs(t *testing.T) {
	env := NewEnvironment()
	if err := env.AddTemplate("a.txt", "A"); err != nil {
		t.Fatalf("add template error: %v", err)
	}
	if err := env.AddTemplate("b.txt", "B"); err != nil {
		t.Fatalf("add template error: %v", err)
	}

	if len(env.Templates()) != 2 {
		t.Fatalf("expected 2 templates, got %d", len(env.Templates()))
	}

	env.RemoveTemplate("a.txt")
	if _, err := env.GetTemplate("a.txt"); err == nil {
		t.Fatal("expected missing template error")
	}

	env.ClearTemplates()
	if len(env.Templates()) != 0 {
		t.Fatalf("expected 0 templates after clear, got %d", len(env.Templates()))
	}
}

func TestPathJoinCallback(t *testing.T) {
	env := NewEnvironment()
	if err := env.AddTemplate("partials/header.html", "Header"); err != nil {
		t.Fatalf("add template error: %v", err)
	}

	env.SetPathJoinCallback(func(name, parent string) string {
		parts := strings.Split(parent, "/")
		if len(parts) > 0 {
			parts = parts[:len(parts)-1]
		}
		for _, segment := range strings.Split(name, "/") {
			switch segment {
			case ".":
				continue
			case "..":
				if len(parts) > 0 {
					parts = parts[:len(parts)-1]
				}
			default:
				parts = append(parts, segment)
			}
		}
		return strings.Join(parts, "/")
	})

	tmpl, err := env.TemplateFromNamedString("pages/home.html", `{% include "../partials/header.html" %}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "Header" {
		t.Errorf("expected 'Header', got %q", result)
	}
}

func TestAutoEscapeDefaults(t *testing.T) {
	env := NewEnvironment()
	var captured AutoEscape
	env.AddFunction("capture", func(state *State, args []Value, kwargs map[string]Value) (Value, error) {
		captured = state.AutoEscape()
		return value.FromString("ok"), nil
	})

	tmplHTML, err := env.TemplateFromNamedString("page.html.j2", "{{ capture() }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	_, err = tmplHTML.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if !captured.IsHTML() {
		t.Fatalf("expected HTML auto-escape, got %#v", captured)
	}

	tmplJSON, err := env.TemplateFromNamedString("data.json", "{{ capture() }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	_, err = tmplJSON.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if !captured.IsJSON() {
		t.Fatalf("expected JSON auto-escape, got %#v", captured)
	}
}

func TestAutoEscapeJSONRendering(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromNamedString("data.json", "{{ value }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"value": "hello"})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != `"hello"` {
		t.Fatalf("expected JSON serialized value, got %q", result)
	}
}

func TestAutoEscapeCustomFormatter(t *testing.T) {
	env := NewEnvironment()
	env.SetAutoEscapeFunc(func(name string) AutoEscape {
		return AutoEscapeCustom("custom")
	})

	tmpl, err := env.TemplateFromString("{{ value }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	_, err = tmpl.Render(map[string]any{"value": "hello"})
	if err == nil {
		t.Fatal("expected render error")
	}
	if templErr, ok := err.(*Error); !ok || templErr.Kind != ErrInvalidOperation {
		t.Fatalf("expected invalid operation error, got %v", err)
	}
}

func TestAutoEscapeJSONBlock(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{% autoescape "json" %}{{ value }}{% endautoescape %}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"value": "hello"})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != `"hello"` {
		t.Fatalf("expected JSON serialized value, got %q", result)
	}
}

func TestFuelTracking(t *testing.T) {
	env := NewEnvironment()
	fuel := uint64(5)
	env.SetFuel(&fuel)

	tmpl, err := env.TemplateFromString("Hello {{ name }}!")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	state, err := tmpl.EvalToState(map[string]any{"name": "World"})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	consumed, remaining, ok := state.FuelLevels()
	if !ok {
		t.Fatal("expected fuel tracking to be enabled")
	}
	if consumed == 0 {
		t.Fatal("expected fuel consumption to be tracked")
	}
	if remaining >= fuel {
		t.Fatalf("expected remaining fuel to decrease, got %d", remaining)
	}
}

func TestOutOfFuel(t *testing.T) {
	env := NewEnvironment()
	fuel := uint64(1)
	env.SetFuel(&fuel)
	tmpl, err := env.TemplateFromString("{{ 42 }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	_, err = tmpl.Render(nil)
	if err == nil {
		t.Fatal("expected out of fuel error")
	}
	if templErr, ok := err.(*Error); !ok || templErr.Kind != ErrOutOfFuel {
		t.Fatalf("expected out of fuel error, got %v", err)
	}
}

func TestRecursionLimit(t *testing.T) {
	env := NewEnvironment()
	env.SetRecursionLimit(1)
	if err := env.AddTemplate("self.html", `{% include "self.html" %}`); err != nil {
		t.Fatalf("add template error: %v", err)
	}

	tmpl, err := env.GetTemplate("self.html")
	if err != nil {
		t.Fatalf("get template error: %v", err)
	}

	_, err = tmpl.Render(nil)
	if err == nil {
		t.Fatal("expected recursion error")
	}
	if tmplErr, ok := err.(*Error); !ok || tmplErr.Kind != ErrBadInclude {
		t.Fatalf("expected bad include error, got %v", err)
	}
}

func TestDebugMode(t *testing.T) {
	env := NewEnvironment()
	env.SetDebug(true)
	tmpl, err := env.TemplateFromNamedString("debug.html", `{{ "a" + 1 }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	_, err = tmpl.Render(nil)
	if err == nil {
		t.Fatal("expected render error")
	}

	templErr, ok := err.(*Error)
	if !ok {
		t.Fatalf("expected template error, got %T", err)
	}
	if templErr.Name != "debug.html" {
		t.Errorf("expected error name to be 'debug.html', got %q", templErr.Name)
	}
	if templErr.Source == "" {
		t.Error("expected error source to be set in debug mode")
	}
}
