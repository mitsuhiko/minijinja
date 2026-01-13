package minijinja

import (
	"strings"
	"sync"

	"github.com/mitsuhiko/minijinja/minijinja-go/lexer"
	"github.com/mitsuhiko/minijinja/minijinja-go/parser"
	"github.com/mitsuhiko/minijinja/minijinja-go/value"
)

// AutoEscape determines the auto-escaping strategy.
type AutoEscape int

const (
	AutoEscapeNone AutoEscape = iota
	AutoEscapeHTML
)

// UndefinedBehavior determines how undefined variables are handled.
type UndefinedBehavior int

const (
	UndefinedLenient UndefinedBehavior = iota
	UndefinedStrict
)

// FilterFunc is the signature for filter functions.
// It receives the value to filter, the arguments, and the state.
type FilterFunc func(state *State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error)

// TestFunc is the signature for test functions.
type TestFunc func(state *State, val value.Value, args []value.Value) (bool, error)

// FunctionFunc is the signature for global functions.
type FunctionFunc func(state *State, args []value.Value, kwargs map[string]value.Value) (value.Value, error)

// LoaderFunc is a function that loads template source by name.
type LoaderFunc func(name string) (string, error)

// AutoEscapeFunc determines auto-escaping based on template name.
type AutoEscapeFunc func(name string) AutoEscape

// Environment holds the configuration and templates.
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

// NewEnvironment creates a new environment with default settings.
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

// EmptyEnvironment creates an environment with no defaults.
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

// AddTemplate adds a template from source.
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

// GetTemplate retrieves a template by name.
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

// TemplateFromString creates a template from source without storing it.
func (e *Environment) TemplateFromString(source string) (*Template, error) {
	return e.TemplateFromNamedString("<string>", source)
}

// TemplateFromNamedString creates a template from source with a name without storing it.
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

// SetLoader sets the template loader function.
func (e *Environment) SetLoader(loader LoaderFunc) {
	e.loader = loader
}

// AddFilter registers a filter function.
func (e *Environment) AddFilter(name string, f FilterFunc) {
	e.filters[name] = f
}

// AddTest registers a test function.
func (e *Environment) AddTest(name string, f TestFunc) {
	e.tests[name] = f
}

// AddFunction registers a global function.
func (e *Environment) AddFunction(name string, f FunctionFunc) {
	e.functions[name] = f
}

// AddGlobal registers a global variable.
func (e *Environment) AddGlobal(name string, v value.Value) {
	e.globals[name] = v
}

// SetAutoEscapeFunc sets the auto-escape callback.
func (e *Environment) SetAutoEscapeFunc(f AutoEscapeFunc) {
	e.autoEscapeFunc = f
}

// SetSyntax sets the syntax configuration.
func (e *Environment) SetSyntax(config lexer.SyntaxConfig) {
	e.syntaxConfig = config
}

// SetWhitespace sets the whitespace configuration.
func (e *Environment) SetWhitespace(config lexer.WhitespaceConfig) {
	e.wsConfig = config
}

// SetUndefinedBehavior sets how undefined variables are handled.
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
type Template struct {
	env      *Environment
	compiled *compiledTemplate
}

// Name returns the template name.
func (t *Template) Name() string {
	return t.compiled.name
}

// Source returns the template source.
func (t *Template) Source() string {
	return t.compiled.source
}

// Render renders the template with the given context.
func (t *Template) Render(ctx any) (string, error) {
	return t.RenderValue(value.FromAny(ctx))
}

// RenderValue renders the template with a Value context.
func (t *Template) RenderValue(ctx value.Value) (string, error) {
	state := newState(t.env, t.compiled.name, t.compiled.source, ctx)
	return state.eval(t.compiled.ast)
}

// EscapeHTML escapes a string for HTML.
// This escapes <, >, &, ", ', and / to match Rust MiniJinja behavior.
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
