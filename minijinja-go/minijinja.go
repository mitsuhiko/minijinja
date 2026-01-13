// Package minijinja provides a Jinja2-compatible template engine for Go.
//
// MiniJinja-Go is a Go port of the MiniJinja template engine, providing
// a powerful and flexible templating system compatible with the Jinja2
// template language.
//
// # Quick Start
//
// Basic usage:
//
//	env := minijinja.NewEnvironment()
//	env.AddTemplate("hello", "Hello {{ name }}!")
//	tmpl, _ := env.GetTemplate("hello")
//	result, _ := tmpl.Render(map[string]any{"name": "World"})
//	fmt.Println(result) // Output: Hello World!
//
// # Template Syntax
//
// For comprehensive documentation about the template syntax, including all
// available tags, filters, tests, and expressions, see the syntax.go file
// or the online documentation.
//
// Key syntax elements:
//   - Variables: {{ variable }}
//   - Blocks: {% if condition %}...{% endif %}
//   - Comments: {# comment #}
//   - Filters: {{ value|filter }}
//   - Tests: {% if value is test %}
//
// # Environment Configuration
//
// The Environment is the central configuration object:
//
//	env := minijinja.NewEnvironment()
//
//	// Add templates
//	env.AddTemplate("base.html", baseTemplate)
//
//	// Configure auto-escaping
//	env.SetAutoEscapeFunc(func(name string) minijinja.AutoEscape {
//	    if strings.HasSuffix(name, ".html") {
//	        return minijinja.AutoEscapeHTML
//	    }
//	    return minijinja.AutoEscapeNone
//	})
//
//	// Add custom filters
//	env.AddFilter("reverse", FilterReverse)
//
//	// Add custom functions
//	env.AddFunction("range", FunctionRange)
//
//	// Configure whitespace handling
//	env.SetTrimBlocks(true)
//	env.SetLstripBlocks(true)
//
// # Custom Filters and Functions
//
// Filters transform values in templates:
//
//	func MyFilter(state *minijinja.State, value minijinja.Value, args []minijinja.Value) (minijinja.Value, error) {
//	    // Transform value
//	    return minijinja.FromString("transformed"), nil
//	}
//	env.AddFilter("myfilter", MyFilter)
//	// In template: {{ value|myfilter }}
//
// Functions can be called from templates:
//
//	func MyFunction(state *minijinja.State, args []minijinja.Value, kwargs map[string]minijinja.Value) (minijinja.Value, error) {
//	    // Process arguments
//	    return minijinja.FromString("result"), nil
//	}
//	env.AddFunction("myfunc", MyFunction)
//	// In template: {{ myfunc(arg1, arg2, key=value) }}
//
// # Error Handling
//
// Template errors provide detailed information:
//
//	tmpl, err := env.GetTemplate("example.html")
//	if err != nil {
//	    if e, ok := err.(*minijinja.Error); ok {
//	        fmt.Printf("Error in %s at line %d: %s\n",
//	            e.Name, e.Span.StartLine, e.Message)
//	    }
//	}
//
// # Value System
//
// The Value type represents dynamically-typed template values:
//
//	// Create values
//	str := minijinja.FromString("hello")
//	num := minijinja.FromInt(42)
//	list := minijinja.FromSlice([]minijinja.Value{str, num})
//	dict := minijinja.FromMap(map[string]minijinja.Value{
//	    "name": str,
//	    "age": num,
//	})
//
//	// Type checking
//	if str.Kind() == minijinja.KindString {
//	    if s, ok := str.AsString(); ok {
//	        fmt.Println(s)
//	    }
//	}
//
// # Template Inheritance
//
// Templates support inheritance via extends and blocks:
//
// Base template (base.html):
//
//	<!DOCTYPE html>
//	<html>
//	{% block head %}
//	  <title>{% block title %}{% endblock %}</title>
//	{% endblock %}
//	<body>
//	  {% block body %}{% endblock %}
//	</body>
//	</html>
//
// Child template:
//
//	{% extends "base.html" %}
//	{% block title %}My Page{% endblock %}
//	{% block body %}
//	  <h1>Hello, World!</h1>
//	{% endblock %}
//
// # Macros
//
// Macros allow reusable template components:
//
//	{% macro render_user(user) %}
//	  <div class="user">
//	    <h3>{{ user.name }}</h3>
//	    <p>{{ user.email }}</p>
//	  </div>
//	{% endmacro %}
//
//	{% for user in users %}
//	  {{ render_user(user) }}
//	{% endfor %}
//
// # See Also
//
//   - syntax.go: Comprehensive syntax documentation
//   - environment.go: Environment configuration
//   - filters.go: Built-in filters
//   - tests.go: Built-in tests
//   - value package: Dynamic value system
package minijinja

// Re-export commonly used types from subpackages
import (
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// Value is a dynamically typed value in the template engine.
type Value = value.Value

// ValueKind describes the type of a Value.
type ValueKind = value.ValueKind

// Common value kinds
const (
	KindUndefined = value.KindUndefined
	KindNone      = value.KindNone
	KindBool      = value.KindBool
	KindNumber    = value.KindNumber
	KindString    = value.KindString
	KindBytes     = value.KindBytes
	KindSeq       = value.KindSeq
	KindMap       = value.KindMap
)

// Value constructors
var (
	Undefined      = value.Undefined
	None           = value.None
	FromBool       = value.FromBool
	FromInt        = value.FromInt
	FromFloat      = value.FromFloat
	FromString     = value.FromString
	FromSafeString = value.FromSafeString
	FromBytes      = value.FromBytes
	FromSlice      = value.FromSlice
	FromMap        = value.FromMap
	FromAny        = value.FromAny
)
