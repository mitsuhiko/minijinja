package minijinja

import (
	"fmt"

	"github.com/mitsuhiko/minijinja/minijinja-go/lexer"
)

// ErrorKind describes the type of error that occurred during template processing.
//
// MiniJinja distinguishes between different error types to help identify the
// source of problems in templates. Each error kind corresponds to a specific
// category of template processing error.
type ErrorKind int

const (
	// ErrSyntax indicates a syntax error in the template.
	//
	// This error occurs when the template contains invalid syntax that cannot
	// be parsed according to the Jinja2 template language rules.
	//
	// Example:
	//     {% if foo %}  // missing endif
	//     {% for %}     // incomplete for loop
	ErrSyntax ErrorKind = iota

	// ErrUndefinedVar indicates an undefined variable was accessed.
	//
	// This error occurs when strict undefined behavior is enabled and a
	// variable that doesn't exist in the context is accessed.
	//
	// Example:
	//     {{ undefined_var }}  // when undefined_var not in context
	ErrUndefinedVar

	// ErrUnknownFilter indicates an unknown filter was used.
	//
	// This error occurs when a template tries to use a filter that has not
	// been registered with the environment.
	//
	// Example:
	//     {{ value|unknown_filter }}
	ErrUnknownFilter

	// ErrUnknownTest indicates an unknown test was used.
	//
	// This error occurs when a template tries to use a test that has not
	// been registered with the environment.
	//
	// Example:
	//     {% if value is unknown_test %}
	ErrUnknownTest

	// ErrUnknownFunction indicates an unknown function was called.
	//
	// This error occurs when a template tries to call a function that has
	// not been registered with the environment.
	//
	// Example:
	//     {{ unknown_function() }}
	ErrUnknownFunction

	// ErrInvalidOperation indicates an invalid operation was attempted.
	//
	// This error occurs for various runtime errors such as:
	//   - Type mismatches in operations (e.g., adding string to number)
	//   - Invalid attribute or item access
	//   - Recursion limit exceeded
	//   - Division by zero
	//
	// Example:
	//     {{ "string" + 42 }}  // cannot add string and number
	ErrInvalidOperation

	// ErrTemplateNotFound indicates a template could not be found.
	//
	// This error occurs when {% include %} or {% extends %} references a
	// template that doesn't exist or can't be loaded.
	//
	// Example:
	//     {% include "missing.html" %}
	ErrTemplateNotFound

	// ErrBadEscape indicates an escaping error occurred.
	//
	// This error is rarely encountered but may occur if custom escape
	// functions fail or produce invalid output.
	ErrBadEscape

	// ErrUnknownBlock indicates an unknown block was referenced.
	//
	// This error occurs when trying to access a block that doesn't exist
	// via super() or self.blockname().
	//
	// Example:
	//     {{ self.nonexistent_block() }}
	ErrUnknownBlock

	// ErrMissingArgument indicates a required argument was not provided.
	//
	// This error occurs when calling a filter, test, function, or macro
	// without providing all required arguments.
	//
	// Example:
	//     {{ value|slice }}  // slice requires start/stop arguments
	ErrMissingArgument

	// ErrTooManyArguments indicates too many arguments were provided.
	//
	// This error occurs when calling a filter, test, function, or macro
	// with more arguments than it accepts.
	//
	// Example:
	//     {{ value|upper("extra") }}  // upper takes no arguments
	ErrTooManyArguments

	// ErrBadInclude indicates an error with template inclusion.
	//
	// This error occurs when {% include %} encounters problems such as
	// invalid template names or recursion issues.
	ErrBadInclude
)

// String returns a human-readable string representation of the error kind.
func (k ErrorKind) String() string {
	switch k {
	case ErrSyntax:
		return "syntax error"
	case ErrUndefinedVar:
		return "undefined variable"
	case ErrUnknownFilter:
		return "unknown filter"
	case ErrUnknownTest:
		return "unknown test"
	case ErrUnknownFunction:
		return "unknown function"
	case ErrInvalidOperation:
		return "invalid operation"
	case ErrTemplateNotFound:
		return "template not found"
	case ErrBadEscape:
		return "bad escape"
	case ErrUnknownBlock:
		return "unknown block"
	case ErrMissingArgument:
		return "missing argument"
	case ErrTooManyArguments:
		return "too many arguments"
	case ErrBadInclude:
		return "bad include"
	default:
		return "error"
	}
}

// Error represents an error that occurred during template processing.
//
// Error provides detailed information about what went wrong, including the
// error kind, a descriptive message, the location in the template source
// where the error occurred, and the template name.
//
// Errors are created internally by the template engine but can also be
// created manually using NewError() for use in custom filters, tests, and
// functions.
//
// Example usage:
//
//	tmpl, err := env.GetTemplate("example.html")
//	if err != nil {
//	    if e, ok := err.(*minijinja.Error); ok {
//	        fmt.Printf("Error in %s at line %d: %s\n",
//	            e.Name, e.Span.StartLine, e.Message)
//	    }
//	}
type Error struct {
	// Kind is the category of error that occurred.
	Kind ErrorKind

	// Message is a human-readable description of what went wrong.
	Message string

	// Span indicates the location in the source where the error occurred.
	// May be nil if location information is not available.
	Span *lexer.Span

	// Name is the template name where the error occurred.
	// May be empty for templates created from strings without names.
	Name string

	// Source is the template source code.
	// Used for error display and debugging.
	Source string
}

// Error returns a formatted error message string.
//
// The format includes the error kind, message, and location information
// if available. The location is shown as "template_name line N" when both
// template name and span are available, or just "line N" when only span
// is available.
//
// Example output:
//     syntax error: unexpected end of template (at example.html line 5)
//     undefined variable: name 'foo' is not defined (at line 12)
//     invalid operation: cannot add string and number
func (e *Error) Error() string {
	if e.Name != "" && e.Span != nil {
		return fmt.Sprintf("%s: %s (at %s line %d)", e.Kind, e.Message, e.Name, e.Span.StartLine)
	}
	if e.Span != nil {
		return fmt.Sprintf("%s: %s (at line %d)", e.Kind, e.Message, e.Span.StartLine)
	}
	return fmt.Sprintf("%s: %s", e.Kind, e.Message)
}

// NewError creates a new error with the given kind and message.
//
// This function is useful when implementing custom filters, tests, or
// functions that need to report errors.
//
// Example:
//
//	func myFilter(state *minijinja.State, value minijinja.Value, args []minijinja.Value) (minijinja.Value, error) {
//	    if len(args) < 1 {
//	        return minijinja.Undefined(), minijinja.NewError(
//	            minijinja.ErrMissingArgument,
//	            "myfilter requires at least 1 argument")
//	    }
//	    // ...
//	}
func NewError(kind ErrorKind, msg string) *Error {
	return &Error{Kind: kind, Message: msg}
}

// WithSpan adds source location information to an error.
//
// This method can be chained when creating errors to add location context.
// It modifies the error in-place and returns the error for chaining.
//
// Example:
//
//	return minijinja.NewError(minijinja.ErrSyntax, "unexpected token").
//	    WithSpan(token.Span())
func (e *Error) WithSpan(span lexer.Span) *Error {
	e.Span = &span
	return e
}

// WithName adds template name information to an error.
//
// This method can be chained when creating errors to add template name context.
// It modifies the error in-place and returns the error for chaining.
//
// Example:
//
//	return minijinja.NewError(minijinja.ErrTemplateNotFound, "missing template").
//	    WithName("layout.html")
func (e *Error) WithName(name string) *Error {
	e.Name = name
	return e
}

// WithSource adds the source code to an error.
//
// This method can be chained when creating errors to add source context for
// better error messages. It modifies the error in-place and returns the error
// for chaining.
//
// Example:
//
//	return minijinja.NewError(minijinja.ErrSyntax, "parse error").
//	    WithSource(templateSource)
func (e *Error) WithSource(source string) *Error {
	e.Source = source
	return e
}
