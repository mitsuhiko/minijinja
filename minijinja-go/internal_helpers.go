package minijinja

import (
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/errors"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
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
)

// NewError creates a new error with the given kind and message.
func NewError(kind ErrorKind, msg string) *Error {
	return errors.NewError(kind, msg)
}

func valueToNative(v value.Value) interface{} {
	switch v.Kind() {
	case value.KindUndefined, value.KindNone:
		return nil
	case value.KindBool:
		b, _ := v.AsBool()
		return b
	case value.KindNumber:
		if i, ok := v.AsInt(); ok && v.IsActualInt() {
			return i
		}
		f, _ := v.AsFloat()
		return f
	case value.KindString:
		s, _ := v.AsString()
		return s
	case value.KindSeq:
		items, _ := v.AsSlice()
		result := make([]interface{}, len(items))
		for i, item := range items {
			result[i] = valueToNative(item)
		}
		return result
	case value.KindMap:
		m, _ := v.AsMap()
		result := make(map[string]interface{}, len(m))
		for k, val := range m {
			result[k] = valueToNative(val)
		}
		return result
	default:
		return v.String()
	}
}
