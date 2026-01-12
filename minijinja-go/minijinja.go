// Package minijinja provides a Jinja2-compatible template engine for Go.
//
// MiniJinja-Go is a Go port of the MiniJinja template engine, providing
// a powerful and flexible templating system compatible with the Jinja2
// template language.
//
// Basic usage:
//
//	env := minijinja.NewEnvironment()
//	env.AddTemplate("hello", "Hello {{ name }}!")
//	tmpl, _ := env.GetTemplate("hello")
//	result, _ := tmpl.Render(map[string]any{"name": "World"})
//	fmt.Println(result) // Output: Hello World!
package minijinja

// Re-export commonly used types from subpackages
import (
	"github.com/mitsuhiko/minijinja/minijinja-go/value"
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
