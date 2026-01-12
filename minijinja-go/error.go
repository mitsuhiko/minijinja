package minijinja

import (
	"fmt"

	"github.com/mitsuhiko/minijinja/minijinja-go/lexer"
)

// ErrorKind describes the type of error.
type ErrorKind int

const (
	ErrSyntax ErrorKind = iota
	ErrUndefinedVar
	ErrUnknownFilter
	ErrUnknownTest
	ErrUnknownFunction
	ErrInvalidOperation
	ErrTemplateNotFound
	ErrBadEscape
	ErrUnknownBlock
	ErrMissingArgument
	ErrTooManyArguments
	ErrBadInclude
)

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
type Error struct {
	Kind    ErrorKind
	Message string
	Span    *lexer.Span
	Name    string // template name
	Source  string // template source (for error display)
}

func (e *Error) Error() string {
	if e.Name != "" && e.Span != nil {
		return fmt.Sprintf("%s: %s (at %s line %d)", e.Kind, e.Message, e.Name, e.Span.StartLine)
	}
	if e.Span != nil {
		return fmt.Sprintf("%s: %s (at line %d)", e.Kind, e.Message, e.Span.StartLine)
	}
	return fmt.Sprintf("%s: %s", e.Kind, e.Message)
}

// NewError creates a new error.
func NewError(kind ErrorKind, msg string) *Error {
	return &Error{Kind: kind, Message: msg}
}

// WithSpan adds span information to an error.
func (e *Error) WithSpan(span lexer.Span) *Error {
	e.Span = &span
	return e
}

// WithName adds template name to an error.
func (e *Error) WithName(name string) *Error {
	e.Name = name
	return e
}

// WithSource adds source to an error.
func (e *Error) WithSource(source string) *Error {
	e.Source = source
	return e
}
