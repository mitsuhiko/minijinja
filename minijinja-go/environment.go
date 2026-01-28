package minijinja

import (
	"context"
	"fmt"
	"io"
	"strings"
	"sync"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/filters"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/parser"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/syntax"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// AutoEscape determines the auto-escaping strategy.
//
// This configures what kind of automatic escaping should happen for a template.
// The default behavior is to look at the file extension to determine the escaping.
type AutoEscape struct {
	kind autoEscapeKind
	name string
}

// autoEscapeKind identifies the auto-escape mode.
type autoEscapeKind int

const (
	autoEscapeNone autoEscapeKind = iota
	autoEscapeHTML
	autoEscapeJSON
	autoEscapeCustom
)

var (
	// AutoEscapeNone disables automatic escaping.
	AutoEscapeNone = AutoEscape{kind: autoEscapeNone}

	// AutoEscapeHTML enables HTML/XML escaping.
	// This escapes <, >, &, ", ', and / characters.
	AutoEscapeHTML = AutoEscape{kind: autoEscapeHTML}

	// AutoEscapeJSON enables JSON escaping/serialization.
	AutoEscapeJSON = AutoEscape{kind: autoEscapeJSON}
)

// AutoEscapeCustom enables a custom auto-escape mode by name.
//
// Custom auto-escape requires a custom formatter to render values.
func AutoEscapeCustom(name string) AutoEscape {
	return AutoEscape{kind: autoEscapeCustom, name: name}
}

// IsNone returns true if auto-escaping is disabled.
func (a AutoEscape) IsNone() bool {
	return a.kind == autoEscapeNone
}

// IsHTML returns true if HTML escaping is enabled.
func (a AutoEscape) IsHTML() bool {
	return a.kind == autoEscapeHTML
}

// IsJSON returns true if JSON escaping is enabled.
func (a AutoEscape) IsJSON() bool {
	return a.kind == autoEscapeJSON
}

// IsCustom returns true if a custom auto-escape mode is active.
func (a AutoEscape) IsCustom() bool {
	return a.kind == autoEscapeCustom
}

// CustomName returns the custom auto-escape name if set.
func (a AutoEscape) CustomName() string {
	return a.name
}

var ignoredAutoEscapeSuffixes = []string{".j2", ".jinja2", ".jinja"}

func defaultAutoEscape(name string) AutoEscape {
	for _, ext := range ignoredAutoEscapeSuffixes {
		if strings.HasSuffix(name, ext) {
			name = strings.TrimSuffix(name, ext)
			break
		}
	}

	dot := strings.LastIndex(name, ".")
	if dot == -1 || dot+1 >= len(name) {
		return AutoEscapeNone
	}

	switch name[dot+1:] {
	case "html", "htm", "xml":
		return AutoEscapeHTML
	case "json", "json5", "js", "yaml", "yml":
		return AutoEscapeJSON
	default:
		return AutoEscapeNone
	}
}

// UndefinedBehavior determines how undefined variables are handled.
//
// This controls the runtime behavior of undefined values in the template engine.
// The type is shared with the value package to allow filters to inspect it.
type UndefinedBehavior = value.UndefinedBehavior

const (
	// UndefinedLenient allows undefined values to be used in templates.
	// They will render as empty strings and can be tested with the 'is defined' test.
	// This is the default behavior and matches Jinja2.
	UndefinedLenient = value.UndefinedLenient

	// UndefinedChainable allows chained access on undefined values without erroring.
	UndefinedChainable = value.UndefinedChainable

	// UndefinedSemiStrict is strict for printing/iteration, but lenient for truthiness.
	UndefinedSemiStrict = value.UndefinedSemiStrict

	// UndefinedStrict causes an error when undefined values are encountered.
	// This is stricter than Jinja2 and helps catch template errors early.
	UndefinedStrict = value.UndefinedStrict
)

// FilterState provides access to filter/test lookup during rendering.
//
// This is implemented by *State and passed to filter and test functions.
type FilterState = filters.State

// TestState provides access to filter/test lookup during rendering.
//
// This is implemented by *State and passed to test functions.
type TestState = filters.State

// FilterFunc is the signature for filter functions.
//
// Filter functions are functions that can be applied to values in templates using
// the pipe operator. They receive the state, the value to filter, positional arguments,
// and keyword arguments, then return a new value or an error.
//
// Example filter that converts to uppercase:
//
//	env.AddFilter("upper", func(state FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
//	    s, err := val.AsString()
//	    if err != nil {
//	        return value.Undefined(), err
//	    }
//	    return value.FromString(strings.ToUpper(s)), nil
//	})
type FilterFunc = filters.FilterFunc

// TestFunc is the signature for test functions.
//
// Test functions perform checks on values where the return value is always true or false.
// They are used with the 'is' operator in templates. They receive the state, the value
// to test, and positional arguments.
//
// Example test that checks if a number is even:
//
//	env.AddTest("even", func(state TestState, val value.Value, args []value.Value) (bool, error) {
//	    n, err := val.AsInt()
//	    if err != nil {
//	        return false, err
//	    }
//	    return n%2 == 0, nil
//	})
type TestFunc = filters.TestFunc

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

// PathJoinFunc is a function that joins template paths.
//
// This is used to resolve relative includes and extends. The first argument
// is the template name being requested, and the second is the parent template
// name that initiated the load.
//
// Example:
//
//	env.SetPathJoinCallback(func(name, parent string) string {
//		parts := strings.Split(parent, "/")
//		if len(parts) > 0 {
//			parts = parts[:len(parts)-1]
//		}
//		for _, segment := range strings.Split(name, "/") {
//			switch segment {
//			case ".":
//				continue
//			case "..":
//				if len(parts) > 0 {
//					parts = parts[:len(parts)-1]
//				}
//			default:
//				parts = append(parts, segment)
//			}
//		}
//		return strings.Join(parts, "/")
//	})
type PathJoinFunc func(name, parent string) string

// AutoEscapeFunc determines auto-escaping based on template name.
//
// This function is invoked when templates are loaded into the environment to determine
// the default auto-escaping behavior. The function is invoked with the name of the
// template and should return the appropriate AutoEscape setting.
//
// The default implementation enables HTML escaping for .html, .htm, and .xml files
// and JSON escaping for .json, .json5, .js, .yaml, and .yml files. The
// .j2/.jinja/.jinja2 suffixes are ignored when determining the mode.
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
	templates         map[string]*compiledTemplate
	templatesMu       sync.RWMutex
	filters           map[string]FilterFunc
	tests             map[string]TestFunc
	globals           map[string]value.Value
	functions         map[string]FunctionFunc
	loader            LoaderFunc
	autoEscapeFunc    AutoEscapeFunc
	pathJoinCallback  PathJoinFunc
	syntaxConfig      syntax.SyntaxConfig
	wsConfig          syntax.WhitespaceConfig
	undefinedBehavior UndefinedBehavior
	recursionLimit    int
	debug             bool
	formatter         FormatterFunc
	fuel              *uint64
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
		templates:         make(map[string]*compiledTemplate),
		filters:           make(map[string]FilterFunc),
		tests:             make(map[string]TestFunc),
		globals:           make(map[string]value.Value),
		functions:         make(map[string]FunctionFunc),
		autoEscapeFunc:    defaultAutoEscape,
		syntaxConfig:      syntax.DefaultSyntax(),
		wsConfig:          syntax.DefaultWhitespace(),
		undefinedBehavior: UndefinedLenient,
		recursionLimit:    maxRecursion,
		fuel:              nil,
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
		syntaxConfig:      syntax.DefaultSyntax(),
		wsConfig:          syntax.DefaultWhitespace(),
		undefinedBehavior: UndefinedLenient,
		recursionLimit:    maxRecursion,
		fuel:              nil,
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
			if templErr, ok := err.(*Error); ok && templErr.Kind == ErrTemplateNotFound {
				return nil, templErr
			}
			return nil, fmt.Errorf("loader error for %q: %w", name, err)
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

// RemoveTemplate removes a template by name from the environment cache.
func (e *Environment) RemoveTemplate(name string) {
	e.templatesMu.Lock()
	delete(e.templates, name)
	e.templatesMu.Unlock()
}

// ClearTemplates removes all templates from the environment cache.
//
// This is useful with loaders to force reloading templates.
func (e *Environment) ClearTemplates() {
	e.templatesMu.Lock()
	e.templates = make(map[string]*compiledTemplate)
	e.templatesMu.Unlock()
}

// Templates returns the currently loaded templates by name.
//
// The returned map is a snapshot copy of the current template cache.
func (e *Environment) Templates() map[string]*Template {
	e.templatesMu.RLock()
	result := make(map[string]*Template, len(e.templates))
	for name, compiled := range e.templates {
		result[name] = &Template{
			env:      e,
			compiled: compiled,
		}
	}
	e.templatesMu.RUnlock()
	return result
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
//	env.AddFilter("shout", func(state FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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
//	env.AddTest("positive", func(state TestState, val value.Value, args []value.Value) (bool, error) {
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

// SetPathJoinCallback sets a callback to join template paths.
//
// This is used to implement relative template resolution for include/extends.
func (e *Environment) SetPathJoinCallback(f PathJoinFunc) {
	e.pathJoinCallback = f
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
//	syntax := syntax.SyntaxConfig{
//	    BlockStart:    "<%",
//	    BlockEnd:      "%>",
//	    VariableStart: "<<",
//	    VariableEnd:   ">>",
//	    CommentStart:  "<#",
//	    CommentEnd:    "#>",
//	}
//	env.SetSyntax(syntax)
func (e *Environment) SetSyntax(config syntax.SyntaxConfig) {
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
//	ws := syntax.WhitespaceConfig{
//	    KeepTrailingNewline: true,
//	    TrimBlocks:          true,
//	    LStripBlocks:        true,
//	}
//	env.SetWhitespace(ws)
func (e *Environment) SetWhitespace(config syntax.WhitespaceConfig) {
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

// SetRecursionLimit sets the maximum recursion depth for template evaluation.
//
// The limit is capped at the default maximum of 500 to avoid stack overflows.
func (e *Environment) SetRecursionLimit(limit int) {
	if limit < 0 {
		limit = 0
	}
	if limit > maxRecursion {
		limit = maxRecursion
	}
	e.recursionLimit = limit
}

// SetDebug enables or disables debug mode.
//
// When enabled, errors include template name and source information to aid debugging.
func (e *Environment) SetDebug(enabled bool) {
	e.debug = enabled
}

// SetFuel sets the optional fuel limit for template evaluation.
//
// When set to a non-nil value, each evaluation step consumes fuel and rendering
// fails with ErrOutOfFuel once the budget is exhausted. Pass nil to disable
// fuel tracking.
func (e *Environment) SetFuel(fuel *uint64) {
	if fuel == nil {
		e.fuel = nil
		return
	}
	val := *fuel
	e.fuel = &val
}

// Fuel returns the configured fuel limit and whether it is enabled.
func (e *Environment) Fuel() (uint64, bool) {
	if e.fuel == nil {
		return 0, false
	}
	return *e.fuel, true
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

func (e *Environment) joinTemplatePath(name, parent string) string {
	if e.pathJoinCallback != nil {
		return e.pathJoinCallback(name, parent)
	}
	return name
}

// Template represents a compiled template.
//
// Templates are created by loading them from the environment using GetTemplate,
// or by parsing strings with TemplateFromString or TemplateFromNamedString.
// Once you have a template, you can render it with a context using Render
// or RenderCtx.
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
// failed. It uses context.Background() internally. For cancellation support
// or passing request-scoped values, use RenderCtx instead.
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
	return t.RenderCtx(context.Background(), ctx)
}

// RenderCtx renders the template with the given context and a context.Context.
//
// The context.Context can be used for cancellation, timeouts, and passing
// request-scoped values to custom filters and functions via State.Context().
//
// Example:
//
//	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
//	defer cancel()
//	output, err := tmpl.RenderCtx(ctx, map[string]any{"name": "World"})
func (t *Template) RenderCtx(goCtx context.Context, ctx any) (string, error) {
	state := newState(goCtx, t.env, t.compiled.name, t.compiled.source, value.FromAny(ctx))
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
	return filters.EscapeHTML(s)
}

// FormatterFunc is the signature for custom output formatters.
//
// A formatter controls how values are converted to strings when output
// in templates. It receives the state, the value to format, and an escape
// function that should be called if the value needs escaping.
//
// Example that treats None as empty string:
//
//	env.SetFormatter(func(state *State, val value.Value, escape func(string) string) string {
//	    if val.IsNone() {
//	        return ""
//	    }
//	    s := val.String()
//	    if !val.IsSafe() {
//	        s = escape(s)
//	    }
//	    return s
//	})
type FormatterFunc func(state *State, val value.Value, escape func(string) string) string

// SetFormatter sets a custom output formatter.
//
// The formatter is called whenever a value is output in a template (i.e., {{ value }}).
// This allows customizing how values are converted to strings and escaped.
//
// Example that treats None as undefined (renders as empty):
//
//	env.SetFormatter(func(state *State, val value.Value, escape func(string) string) string {
//	    if val.IsNone() {
//	        return ""  // Treat None like undefined
//	    }
//	    s := val.String()
//	    if !val.IsSafe() {
//	        s = escape(s)
//	    }
//	    return s
//	})
func (e *Environment) SetFormatter(f FormatterFunc) {
	e.formatter = f
}

// EvalToState evaluates the template and returns its state.
//
// This is useful when you need to:
//   - Render specific blocks of a template
//   - Call macros programmatically
//   - Inspect template exports
//
// The template is evaluated but the output is discarded. You can then
// use the returned State to render blocks, call macros, etc.
//
// Example:
//
//	env := NewEnvironment()
//	env.AddTemplate("page.html", `
//	    {% block title %}Default Title{% endblock %}
//	    {% block body %}Default Body{% endblock %}
//	    {% macro greet(name) %}Hello {{ name }}!{% endmacro %}
//	    {% set version = "1.0" %}
//	`)
//	tmpl, _ := env.GetTemplate("page.html")
//	state, _ := tmpl.EvalToState(map[string]any{"user": "John"})
//
//	title, _ := state.RenderBlock("title")
//	greeting, _ := state.CallMacro("greet", value.FromString("World"))
//	exports := state.Exports()
func (t *Template) EvalToState(ctx any) (*State, error) {
	return t.EvalToStateCtx(context.Background(), ctx)
}

// EvalToStateCtx evaluates the template with a context.Context and returns its state.
func (t *Template) EvalToStateCtx(goCtx context.Context, ctx any) (*State, error) {
	state := newState(goCtx, t.env, t.compiled.name, t.compiled.source, value.FromAny(ctx))

	// Evaluate the template (this populates blocks, macros, and variables)
	_, err := state.eval(t.compiled.ast)
	if err != nil {
		return nil, err
	}

	return state, nil
}

// RenderToWrite renders the template and writes the output to the given writer.
//
// This is useful for streaming output directly to a response writer or file
// without buffering the entire output in memory.
//
// Example:
//
//	tmpl, _ := env.GetTemplate("large_report.html")
//	file, _ := os.Create("report.html")
//	defer file.Close()
//	err := tmpl.RenderToWrite(map[string]any{"data": largeData}, file)
func (t *Template) RenderToWrite(ctx any, w io.Writer) error {
	return t.RenderToWriteCtx(context.Background(), ctx, w)
}

// RenderToWriteCtx renders the template with a context.Context and writes to the given writer.
func (t *Template) RenderToWriteCtx(goCtx context.Context, ctx any, w io.Writer) error {
	state := newState(goCtx, t.env, t.compiled.name, t.compiled.source, value.FromAny(ctx))
	state.out = ioStringWriter{w: w}
	_, err := state.eval(t.compiled.ast)
	return err
}
