// Example: macros
//
// This example demonstrates using macros and imports in templates.
// Macros allow you to create reusable template components.
package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

func main() {
	baseDir, err := findExampleDir()
	if err != nil {
		log.Fatal(err)
	}

	macrosHTML, err := os.ReadFile(filepath.Join(baseDir, "macros.html"))
	if err != nil {
		log.Fatal(err)
	}

	templateHTML, err := os.ReadFile(filepath.Join(baseDir, "template.html"))
	if err != nil {
		log.Fatal(err)
	}

	env := minijinja.NewEnvironment()

	err = env.AddTemplate("macros.html", string(macrosHTML))
	if err != nil {
		log.Fatal(err)
	}

	err = env.AddTemplate("template.html", string(templateHTML))
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("template.html")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(map[string]any{
		"username": "John Doe",
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "macros"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "template.html")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate macros example files")
}
