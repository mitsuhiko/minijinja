// Example: Rendering with a value.Value context
//
// This example demonstrates that a value.Value can be used directly as the
// rendering context.
package main

import (
	"fmt"
	"log"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func main() {
	env := minijinja.NewEnvironment()

	ctx := value.FromMap(map[string]value.Value{
		"name": value.FromString("Peter"),
	})

	tmpl, err := env.TemplateFromString("Hello {{ name }}!")
	if err != nil {
		log.Fatalf("Failed to parse template: %v", err)
	}

	result, err := tmpl.Render(ctx)
	if err != nil {
		log.Fatalf("Failed to render: %v", err)
	}

	fmt.Println(result)
}
