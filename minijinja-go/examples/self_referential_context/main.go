// Example: self-referential-context
//
// This example demonstrates creating a context object that includes
// a reference to itself. This allows templates to access the entire
// context via a special variable (CONTEXT).
//
// Unlike a simple map copy, this uses MakeObjectMap to create a dynamic
// wrapper that intercepts attribute access, providing true self-reference
// without copying the context.
package main

import (
	"fmt"
	"iter"
	"log"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// MakeSelfReferential wraps a context value so that accessing "CONTEXT"
// returns the wrapped context itself. This allows templates to access
// the entire context via CONTEXT.key while CONTEXT.CONTEXT remains undefined.
func MakeSelfReferential(ctx value.Value) value.Value {
	// Get the keys from the original context for enumeration
	enumerate := func() iter.Seq[value.Value] {
		return func(yield func(value.Value) bool) {
			// First yield all keys from the original context
			for _, k := range ctx.Iter() {
				if !yield(k) {
					return
				}
			}
			// Then yield "CONTEXT" if not already in the context
			if ctx.GetAttr("CONTEXT").IsUndefined() {
				yield(value.FromString("CONTEXT"))
			}
		}
	}

	getAttr := func(key value.Value) value.Value {
		if s, ok := key.AsString(); ok && s == "CONTEXT" {
			// Return the wrapped context (not the wrapper itself)
			// This allows CONTEXT.name to work while CONTEXT.CONTEXT is undefined
			return ctx
		}
		// Delegate to the wrapped context
		v := ctx.GetAttr(key.String())
		if !v.IsUndefined() {
			return v
		}
		return value.Undefined()
	}

	return value.MakeObjectMap(enumerate, getAttr)
}

const template = `
name: {{ name }}
CONTEXT.name: {{ CONTEXT.name }}
CONTEXT.CONTEXT is undefined: {{ CONTEXT.CONTEXT is undefined }}
CONTEXT: {{ CONTEXT }}
`

func main() {
	env := minijinja.NewEnvironment()

	tmpl, err := env.TemplateFromString(template)
	if err != nil {
		log.Fatal(err)
	}

	// Create the context with self-reference
	ctx := MakeSelfReferential(value.FromMap(map[string]value.Value{
		"name":        value.FromString("John"),
		"other_value": value.FromInt(42),
	}))

	result, err := tmpl.Render(ctx)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
