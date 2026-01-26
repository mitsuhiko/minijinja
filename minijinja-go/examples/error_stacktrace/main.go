// Example: error stacktrace
//
// This example demonstrates how to render detailed error output, including
// template snippets, referenced locals, and chained include errors.
package main

import (
	"fmt"
	"log"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

func main() {
	env := minijinja.NewEnvironment()
	env.SetDebug(true)

	if err := env.AddTemplate("base.html", `Hello {% include "partial.html" %}`); err != nil {
		log.Fatal(err)
	}
	if err := env.AddTemplate("partial.html", `{{ name + 1 }}`); err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("base.html")
	if err != nil {
		log.Fatal(err)
	}

	_, err = tmpl.Render(map[string]any{"name": "World"})
	if err != nil {
		fmt.Printf("Render failed:\n%#v\n", err)
	}
}
