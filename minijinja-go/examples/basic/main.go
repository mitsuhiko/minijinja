// Example: Basic template rendering
//
// This example demonstrates the fundamental usage of MiniJinja for Go,
// including creating an environment, adding templates, and rendering
// with a context.
package main

import (
	"fmt"
	"log"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

func main() {
	// Create a new environment with default settings.
	// This includes all built-in filters, tests, and functions.
	env := minijinja.NewEnvironment()

	// Add a template to the environment.
	// Templates are parsed and stored when added.
	err := env.AddTemplate("greeting.txt", `Hello {{ name }}!

Your items:
{% for item in items %}
  - {{ item }}
{% endfor %}

{% if premium %}
You are a premium member!
{% else %}
Upgrade to premium for more features.
{% endif %}
`)
	if err != nil {
		log.Fatalf("Failed to add template: %v", err)
	}

	// Get the template from the environment.
	tmpl, err := env.GetTemplate("greeting.txt")
	if err != nil {
		log.Fatalf("Failed to get template: %v", err)
	}

	// Render the template with a context.
	// The context can be a map[string]any, struct, or any Go value.
	result, err := tmpl.Render(map[string]any{
		"name":    "Alice",
		"items":   []string{"apples", "oranges", "bananas"},
		"premium": true,
	})
	if err != nil {
		log.Fatalf("Failed to render: %v", err)
	}

	fmt.Println(result)

	// You can also render templates from strings without storing them:
	tmpl2, err := env.TemplateFromString("Quick: {{ 1 + 2 }} = three")
	if err != nil {
		log.Fatalf("Failed to create template: %v", err)
	}

	result2, err := tmpl2.Render(nil)
	if err != nil {
		log.Fatalf("Failed to render: %v", err)
	}

	fmt.Println(result2)
}
