package minijinja

import (
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/errors"
)

// Error represents an error that occurred during template processing.
type Error = errors.Error

// ErrorKind describes the type of error that occurred during template processing.
type ErrorKind = errors.ErrorKind

const (
	ErrSyntax           = errors.ErrSyntax
	ErrUndefinedVar     = errors.ErrUndefinedVar
	ErrUnknownFilter    = errors.ErrUnknownFilter
	ErrUnknownTest      = errors.ErrUnknownTest
	ErrUnknownFunction  = errors.ErrUnknownFunction
	ErrInvalidOperation = errors.ErrInvalidOperation
	ErrTemplateNotFound = errors.ErrTemplateNotFound
	ErrBadEscape        = errors.ErrBadEscape
	ErrUnknownBlock     = errors.ErrUnknownBlock
	ErrMissingArgument  = errors.ErrMissingArgument
	ErrTooManyArguments = errors.ErrTooManyArguments
	ErrBadInclude       = errors.ErrBadInclude
	ErrOutOfFuel        = errors.ErrOutOfFuel
	ErrEvalBlock        = errors.ErrEvalBlock
	ErrUnknownMethod    = errors.ErrUnknownMethod
)

// NewError creates a new error with the given kind and message.
func NewError(kind ErrorKind, msg string) *Error {
	return errors.NewError(kind, msg)
}
