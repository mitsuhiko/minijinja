package minijinja

import (
	"strings"
	"sync"

	"github.com/mitsuhiko/minijinja/minijinja-go/lexer"
	"github.com/mitsuhiko/minijinja/minijinja-go/parser"
	"github.com/mitsuhiko/minijinja/minijinja-go/value"
)

// AutoEscape determines the auto-escaping strategy.
//
// This configures what kind of automatic escaping should happen for a template.
// The default behavior is to look at the file extension to determine the escaping.
type AutoEscape int

const (
	// AutoEscapeNone disables automatic escaping.
	AutoEscapeNone AutoEscape = iota
	
	// AutoEscapeHTML enables HTML/XML escaping.
	// This escapes <, >, &, ", ', and / characters.
	AutoEscapeHTML
)

// UndefinedBehavior determines how undefined variables are handled.
//
// This controls the runtime behavior of undefined values in the template engine.
type UndefinedBehavior int

const (
	// UndefinedLenient allows undefined values to be used in templates.
	// They will render as empty strings and can be tested with the 'is defined' test.
	// This is the default behavior and matches Jinja2.
	UndefinedLenient UndefinedBehavior = iota
	
	// UndefinedStrict causes an error when undefined values are encountered.
	// This is stricter than Jinja2 and helps catch template errors early.
	UndefinedStrict
)

// FilterFunc is the signature for filter functions.
//
// Filter functions are functions that can be applied to values in templates using
// the pipe operator. They receive the state, the value to filter, positional arguments,
// and keyword arguments, then return a new value or an error.
//
// Example filter that converts to uppercase:
//
//	env.AddFilter("upper", func(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
//	    s, err := val.AsString()
//	    if err != nil {
//	        return value.Undefined(), err
//	    }
//	    return value.FromString(strings.ToUpper(s)), nil
//	})
type FilterFunc func(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error)

// TestFunc is the signature for test functions.
//
// Test functions perform checks on values where the return value is always true or false.
// They are used with the 'is' operator in templates. They receive the state, the value
// to test, and positional arguments.
//
// Example test that checks if a number is even:
//
//	env.AddTest("even", func(state *State, val value.Value, args []value.Value) (bool, error) {
//	    n, err := val.AsInt()
//	    if err != nil {
//	        return false, err
//	    }
//	    return n%2 == 0, nil
//	})
type TestFunc func(state *State, val value.Value, args []value.Value) (bool, error)

// FunctionFunc is the signature for global functions.
//
// Global functions can be called from templates like regular functions. They receive
// the state, positional arguments, and keyword arguments, then return a value or an error.
// Functions and other global variables share the same namespace.
//
// Example function that returns a range:
//
//	env.AddFunction("range", func(state *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
//	    if len(args) != 1 {
//	        return value.Undefined(), errors.New("range expects 1 argument")
//	    }
//	    n, err := args[0].AsInt()
//	    if err != nil {
//	        return value.Undefined(), err
//	    }
//	    result := make([]value.Value, n)
//	    for i := 0; i < n; i++ {
//	        result[i] = value.FromInt(i)
//	    }
//	    return value.FromSlice(result), nil
//	})
type FunctionFunc func(state *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error)

// LoaderFunc is a function that loads template source by name.
//
// When a template loader is registered, the environment gains the ability to dynamically
// load templates. The loader is invoked with the name of the template. If the template
// exists, it should return the source; otherwise, it should return an error.
//
// Once a template has been loaded it's cached in the environment. This means the loader
// is only invoked once per template name unless the template is explicitly removed.
//
// Example loader that loads from a map:
//
//	templates := map[string]string{
//	    "index.html": "<h1>Hello {{ name }}</h1>",
//	    "base.html":  "<!DOCTYPE html>...",
//	}
//	env.SetLoader(func(name string) (string, error) {
//	    if tmpl, ok := templates[name]; ok {
//	        return tmpl, nil
//	    }
//	    return "", fmt.Errorf("template not found: %s", name)
//	})
type LoaderFunc func(name string) (string, error)

// AutoEscapeFunc determines auto-escaping based on template name.
//
// This function is invoked when templates are loaded into the environment to determine
// the default auto-escaping behavior. The function is invoked with the name of the
// template and should return the appropriate AutoEscape setting.
//
// The default implementation enables HTML escaping for .html, .htm, and .xml files.
//
// Example that enables escaping for SVG files:
//
//	env.SetAutoEscapeFunc(func(name string) AutoEscape {
//	    if strings.HasSuffix(name, ".html") || strings.HasSuffix(name, ".htm") ||
//	       strings.HasSuffix(name, ".xml") || strings.HasSuffix(name, ".svg") {
//	        return AutoEscapeHTML
//	    }
//	    return AutoEscapeNone
//	})
type AutoEscapeFunc func(name string) AutoEscape

// Environment holds the engine configuration.
//
// This object holds the central configuration state for templates. It is also
// the container for all loaded templates.
//
// There are generally two ways to construct an environment:
//
// - NewEnvironment creates an environment preconfigured with sensible defaults.
// It will contain all built-in filters, tests, and globals, as well as a callback
// for auto-escaping based on file extension.
//
// - EmptyEnvironment creates a completely blank environment with no filters,
// tests, globals, or default auto-escaping logic.
type Environment struct {
	templates      map[string]*compiledTemplate
	templatesMu    sync.RWMutex
	filters        map[string]FilterFunc
	tests          map[string]TestFunc
	globals        map[string]value.Value
	functions      map[string]FunctionFunc
	loader         LoaderFunc
	autoEscapeFunc AutoEscapeFunc
	syntaxConfig      lexer.SyntaxConfig
	wsConfig          lexer.WhitespaceConfig
	undefinedBehavior UndefinedBehavior
}

type compiledTemplate struct {
	name   string
	source string
	ast    *parser.Template
}

// NewEnvironment creates a new environment with sensible defaults.
//
// This environment does not yet contain any templates but it will have all
// the default filters, tests, and globals loaded. If you do not want any
// default configuration you can use the alternative EmptyEnvironment function.
//
// The default configuration includes:
//   - Auto-escaping enabled for .html, .htm, and .xml files
//   - All built-in filters (upper, lower, length, etc.)
//   - All built-in tests (defined, undefined, even, odd, etc.)
//   - All built-in global functions (range, dict, etc.)
//   - Lenient undefined behavior
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTemplate("hello.html", "Hello {{ name }}!")
//	tmpl, _ := env.GetTemplate("hello.html")
//	output, _ := tmpl.Render(map[string]any{"name": "World"})
//	// output: "Hello World!"
func NewEnvironment() *Environment {
	env := &Environment{
		templates: make(map[string]*compiledTemplate),
		filters:   make(map[string]FilterFunc),
		tests:     make(map[string]TestFunc),
		globals:   make(map[string]value.Value),
		functions: make(map[string]FunctionFunc),
		autoEscapeFunc: func(name string) AutoEscape {
			// Default: escape HTML files
			if len(name) > 5 && name[len(name)-5:] == ".html" {
				return AutoEscapeHTML
			}
			if len(name) > 4 && name[len(name)-4:] == ".htm" {
				return AutoEscapeHTML
			}
			if len(name) > 4 && name[len(name)-4:] == ".xml" {
				return AutoEscapeHTML
			}
			return AutoEscapeNone
		},
		syntaxConfig:      lexer.DefaultSyntax(),
		wsConfig:          lexer.DefaultWhitespace(),
		undefinedBehavior: UndefinedLenient,
	}

	// Register default filters
	registerDefaultFilters(env)
	// Register default tests
	registerDefaultTests(env)
	// Register default functions
	registerDefaultFunctions(env)

	return env
}

// EmptyEnvironment creates a completely empty environment.
//
// This environment has no filters, no templates, no globals, and no default
// logic for auto-escaping configured. This is useful when you want complete
// control over the environment configuration.
//
// Example:
//
//	env := EmptyEnvironment()
//	// Add only the filters and functions you need
//	env.AddFilter("myfilter", myFilterFunc)
//	env.AddFunction("myfunc", myFunctionFunc)
func EmptyEnvironment() *Environment {
	return &Environment{
		templates: make(map[string]*compiledTemplate),
		filters:   make(map[string]FilterFunc),
		tests:     make(map[string]TestFunc),
		globals:   make(map[string]value.Value),
		functions: make(map[string]FunctionFunc),
		autoEscapeFunc: func(name string) AutoEscape {
			return AutoEscapeNone
		},
		syntaxConfig:      lexer.DefaultSyntax(),
		wsConfig:          lexer.DefaultWhitespace(),
		undefinedBehavior: UndefinedLenient,
	}
}

// AddTemplate loads a template from a string into the environment.
//
// The name parameter defines the name of the template which identifies it.
// To look up a loaded template use the GetTemplate method.
//
// This method fails if the template has a syntax error. Templates are parsed
// when they are added to the environment.
//
// Example:
//
//	env := NewEnvironment()
//	err := env.AddTemplate("index.html", "Hello {{ name }}!")
//	if err != nil {
//	    log.Fatal(err)
//	}
//	tmpl, _ := env.GetTemplate("index.html")
func (e *Environment) AddTemplate(name, source string) error {
	ast, err := parser.Parse(source, name, e.syntaxConfig, e.wsConfig)
	if err != nil {
		return err
	}

	e.templatesMu.Lock()
	e.templates[name] = &compiledTemplate{
		name:   name,
		source: source,
		ast:    ast,
	}
	e.templatesMu.Unlock()
	return nil
}

// GetTemplate fetches a template by name.
//
// This requires that the template has been loaded with AddTemplate beforehand,
// or that a loader has been configured with SetLoader. If the template was not
// loaded and no loader is available, an error of kind ErrTemplateNotFound is returned.
//
// If a loader is configured, it will be invoked to dynamically load the template
// on first access. Once loaded, the template is cached in the environment.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTemplate("hello.txt", "Hello {{ name }}!")
//	tmpl, err := env.GetTemplate("hello.txt")
//	if err != nil {
//	    log.Fatal(err)
//	}
//	output, _ := tmpl.Render(map[string]any{"name": "World"})
//	fmt.Println(output) // Output: Hello World!
func (e *Environment) GetTemplate(name string) (*Template, error) {
	e.templatesMu.RLock()
	compiled, ok := e.templates[name]
	e.templatesMu.RUnlock()

	if ok {
		return &Template{
			env:      e,
			compiled: compiled,
		}, nil
	}

	// Try loader
	if e.loader != nil {
		source, err := e.loader(name)
		if err != nil {
			return nil, NewError(ErrTemplateNotFound, name)
		}
		if err := e.AddTemplate(name, source); err != nil {
			return nil, err
		}
		e.templatesMu.RLock()
		compiled = e.templates[name]
		e.templatesMu.RUnlock()
		return &Template{
			env:      e,
			compiled: compiled,
		}, nil
	}

	return nil, NewError(ErrTemplateNotFound, name)
}

// TemplateFromString creates a template from a string without storing it.
//
// This is useful when you need to render a template only once. The internal
// name of the template is set to "<string>".
//
// This method is a shortcut for TemplateFromNamedString with the name set to "<string>".
//
// Example:
//
//	env := NewEnvironment()
//	tmpl, err := env.TemplateFromString("Hello {{ name }}!")
//	if err != nil {
//	    log.Fatal(err)
//	}
//	output, _ := tmpl.Render(map[string]any{"name": "World"})
//	fmt.Println(output) // Output: Hello World!
func (e *Environment) TemplateFromString(source string) (*Template, error) {
	return e.TemplateFromNamedString("<string>", source)
}

// TemplateFromNamedString creates a template from a string with a specific name.
//
// Like TemplateFromString, but allows you to specify the name of the template.
// The template is not stored in the environment and must be used directly.
// This is useful for one-off template rendering where you want a meaningful
// name for error messages.
//
// Example:
//
//	env := NewEnvironment()
//	tmpl, err := env.TemplateFromNamedString("greeting", "Hello {{ name }}!")
//	if err != nil {
//	    log.Fatal(err)
//	}
//	output, _ := tmpl.Render(map[string]any{"name": "World"})
func (e *Environment) TemplateFromNamedString(name, source string) (*Template, error) {
	ast, err := parser.Parse(source, name, e.syntaxConfig, e.wsConfig)
	if err != nil {
		return nil, err
	}

	return &Template{
		env: e,
		compiled: &compiledTemplate{
			name:   name,
			source: source,
			ast:    ast,
		},
	}, nil
}

// SetLoader registers a template loader as the source of templates.
//
// When a template loader is registered, the environment gains the ability
// to dynamically load templates. The loader is invoked with the name of
// the template when GetTemplate is called and the template is not already
// cached. Once a template has been loaded, it's stored in the environment,
// so the loader is only invoked once per template name.
//
// Example loading from a directory:
//
//	env := NewEnvironment()
//	env.SetLoader(func(name string) (string, error) {
//	    content, err := os.ReadFile(filepath.Join("templates", name))
//	    if err != nil {
//	        return "", err
//	    }
//	    return string(content), nil
//	})
//	tmpl, _ := env.GetTemplate("index.html")
func (e *Environment) SetLoader(loader LoaderFunc) {
	e.loader = loader
}

// AddFilter registers a filter function.
//
// Filter functions are functions that can be applied to values in templates
// using the pipe operator (|). For more details about filters, see the
// FilterFunc type documentation.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFilter("shout", func(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
//	    s, err := val.AsString()
//	    if err != nil {
//	        return value.Undefined(), err
//	    }
//	    return value.FromString(strings.ToUpper(s) + "!"), nil
//	})
//	// In template: {{ "hello"|shout }} renders as: HELLO!
func (e *Environment) AddFilter(name string, f FilterFunc) {
	e.filters[name] = f
}

// AddTest registers a test function.
//
// Test functions are similar to filters but perform a check on a value
// where the return value is always true or false. They are used with the
// 'is' operator in templates. For more details about tests, see the
// TestFunc type documentation.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTest("positive", func(state *State, val value.Value, args []value.Value) (bool, error) {
//	    n, err := val.AsInt()
//	    if err != nil {
//	        return false, err
//	    }
//	    return n > 0, nil
//	})
//	// In template: {% if value is positive %}...{% endif %}
func (e *Environment) AddTest(name string, f TestFunc) {
	e.tests[name] = f
}

// AddFunction registers a global function.
//
// Global functions can be called from templates like regular functions.
// Functions and other global variables share the same namespace.
// For more details about functions, see the FunctionFunc type documentation.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddFunction("greet", func(state *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
//	    if len(args) != 1 {
//	        return value.Undefined(), errors.New("greet expects 1 argument")
//	    }
//	    name, err := args[0].AsString()
//	    if err != nil {
//	        return value.Undefined(), err
//	    }
//	    return value.FromString("Hello, " + name + "!"), nil
//	})
//	// In template: {{ greet("World") }} renders as: Hello, World!
func (e *Environment) AddFunction(name string, f FunctionFunc) {
	e.functions[name] = f
}

// AddGlobal registers a global variable.
//
// Global variables are available in all templates. Note that functions
// and other global variables share the same namespace, so you cannot
// have a global variable and a function with the same name.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddGlobal("site_name", value.FromString("My Website"))
//	env.AddGlobal("version", value.FromString("1.0.0"))
//	// In template: {{ site_name }} v{{ version }}
func (e *Environment) AddGlobal(name string, v value.Value) {
	e.globals[name] = v
}

// SetAutoEscapeFunc sets a new function to select the default auto-escaping.
//
// This function is invoked when templates are loaded into the environment
// to determine the default auto-escaping behavior. The function is invoked
// with the name of the template and can make an initial auto-escaping
// decision based on that. The default implementation enables HTML escaping
// for .html, .htm, and .xml files.
//
// Example:
//
//	env := NewEnvironment()
//	env.SetAutoEscapeFunc(func(name string) AutoEscape {
//	    if strings.HasSuffix(name, ".html") || strings.HasSuffix(name, ".htm") {
//	        return AutoEscapeHTML
//	    }
//	    return AutoEscapeNone
//	})
func (e *Environment) SetAutoEscapeFunc(f AutoEscapeFunc) {
	e.autoEscapeFunc = f
}

// SetSyntax sets the syntax configuration for the environment.
//
// This setting is used whenever a template is loaded into the environment.
// Changing it at a later point only affects future templates loaded.
// The syntax configuration controls the delimiters used for template tags,
// variables, and comments.
//
// Example:
//
//	env := NewEnvironment()
//	syntax := lexer.SyntaxConfig{
//	    BlockStart:    "<%",
//	    BlockEnd:      "%>",
//	    VariableStart: "<<",
//	    VariableEnd:   ">>",
//	    CommentStart:  "<#",
//	    CommentEnd:    "#>",
//	}
//	env.SetSyntax(syntax)
func (e *Environment) SetSyntax(config lexer.SyntaxConfig) {
	e.syntaxConfig = config
}

// SetWhitespace sets the whitespace handling configuration.
//
// This setting is used whenever a template is loaded into the environment.
// Changing it at a later point only affects future templates loaded.
// The whitespace configuration controls how whitespace around template
// tags is handled.
//
// Example:
//
//	env := NewEnvironment()
//	ws := lexer.WhitespaceConfig{
//	    KeepTrailingNewline: true,
//	    TrimBlocks:          true,
//	    LStripBlocks:        true,
//	}
//	env.SetWhitespace(ws)
func (e *Environment) SetWhitespace(config lexer.WhitespaceConfig) {
	e.wsConfig = config
}

// SetUndefinedBehavior changes the undefined behavior.
//
// This changes the runtime behavior of undefined values in the template
// engine. For more information see UndefinedBehavior. The default is
// UndefinedLenient.
//
// Example:
//
//	env := NewEnvironment()
//	env.SetUndefinedBehavior(UndefinedStrict)
//	// Now any undefined variable will cause an error
func (e *Environment) SetUndefinedBehavior(behavior UndefinedBehavior) {
	e.undefinedBehavior = behavior
}

// getFilter returns a filter by name.
func (e *Environment) getFilter(name string) (FilterFunc, bool) {
	f, ok := e.filters[name]
	return f, ok
}

// getTest returns a test by name.
func (e *Environment) getTest(name string) (TestFunc, bool) {
	t, ok := e.tests[name]
	return t, ok
}

// getFunction returns a function by name.
func (e *Environment) getFunction(name string) (FunctionFunc, bool) {
	f, ok := e.functions[name]
	return f, ok
}

// getGlobal returns a global by name.
func (e *Environment) getGlobal(name string) (value.Value, bool) {
	v, ok := e.globals[name]
	return v, ok
}

// Template represents a compiled template.
//
// Templates are created by loading them from the environment using GetTemplate,
// or by parsing strings with TemplateFromString or TemplateFromNamedString.
// Once you have a template, you can render it with a context using the Render
// or RenderValue methods.
type Template struct {
	env      *Environment
	compiled *compiledTemplate
}

// Name returns the name of the template.
//
// This is the name that was used when the template was loaded or created.
// For templates created with TemplateFromString, this will be "<string>".
func (t *Template) Name() string {
	return t.compiled.name
}

// Source returns the original template source code.
//
// This returns the raw template source that was used to create this template.
// This can be useful for debugging or displaying the template contents.
func (t *Template) Source() string {
	return t.compiled.source
}

// Render renders the template with the given context.
//
// The context can be any Go value that will be converted to a template value.
// Commonly this is a map[string]any, struct, or any type that can be converted
// to a template value.
//
// This method returns the rendered output as a string, or an error if rendering
// failed.
//
// Example:
//
//	env := NewEnvironment()
//	tmpl, _ := env.TemplateFromString("Hello {{ name }}!")
//	output, err := tmpl.Render(map[string]any{"name": "World"})
//	if err != nil {
//	    log.Fatal(err)
//	}
//	fmt.Println(output) // Output: Hello World!
func (t *Template) Render(ctx any) (string, error) {
	return t.RenderValue(value.FromAny(ctx))
}

// RenderValue renders the template with a Value context.
//
// This is like Render but takes a value.Value directly instead of converting
// from a Go value. This can be more efficient if you already have a Value
// or need more control over the conversion process.
//
// Example:
//
//	env := NewEnvironment()
//	tmpl, _ := env.TemplateFromString("Hello {{ name }}!")
//	ctx := value.FromMap(map[string]value.Value{
//	    "name": value.FromString("World"),
//	})
//	output, _ := tmpl.RenderValue(ctx)
//	fmt.Println(output) // Output: Hello World!
func (t *Template) RenderValue(ctx value.Value) (string, error) {
	state := newState(t.env, t.compiled.name, t.compiled.source, ctx)
	return state.eval(t.compiled.ast)
}

// EscapeHTML escapes a string for safe use in HTML.
//
// This function escapes the following characters to their HTML entity equivalents:
//   - < becomes &lt;
//   - > becomes &gt;
//   - & becomes &amp;
//   - " becomes &quot;
//   - ' becomes &#x27;
//   - / becomes &#x2f;
//
// This matches the escaping behavior of the Rust MiniJinja implementation.
// The escaping of / helps prevent escaping out of inline script or style tags.
//
// This function is used internally by the auto-escaping mechanism when
// AutoEscapeHTML is enabled, but it can also be called directly if needed.
//
// Example:
//
//	escaped := EscapeHTML("<script>alert('XSS')</script>")
//	// Result: &lt;script&gt;alert(&#x27;XSS&#x27;)&lt;&#x2f;script&gt;
func EscapeHTML(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	for _, r := range s {
		switch r {
		case '<':
			b.WriteString("&lt;")
		case '>':
			b.WriteString("&gt;")
		case '&':
			b.WriteString("&amp;")
		case '"':
			b.WriteString("&quot;")
		case '\'':
			b.WriteString("&#x27;")
		case '/':
			b.WriteString("&#x2f;")
		default:
			b.WriteRune(r)
		}
	}
	return b.String()
}
