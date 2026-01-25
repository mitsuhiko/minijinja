// Example: self-referential-context
//
// This example demonstrates creating a context object that includes
// a reference to itself. This allows templates to access the entire
// context via a special variable (CONTEXT).
package main

import (
	"fmt"
	"log"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// MakeSelfReferential creates a context map that includes a CONTEXT
// key pointing to the original context.
func MakeSelfReferential(ctx map[string]value.Value) map[string]value.Value {
	// Create a copy of the context
	result := make(map[string]value.Value, len(ctx)+1)
	for k, v := range ctx {
		result[k] = v
	}
	// Add CONTEXT pointing to the original (not including CONTEXT itself)
	result["CONTEXT"] = value.FromMap(ctx)
	return result
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
	ctx := MakeSelfReferential(map[string]value.Value{
		"name":        value.FromString("John"),
		"other_value": value.FromInt(42),
	})

	result, err := tmpl.Render(ctx)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
