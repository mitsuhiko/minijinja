// Example: debug
//
// This example demonstrates the debug() function when debug mode is enabled.
package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

func main() {
	baseDir, err := findExampleDir()
	if err != nil {
		log.Fatal(err)
	}

	templatePath := filepath.Join(baseDir, "demo.txt")
	contents, err := os.ReadFile(templatePath)
	if err != nil {
		log.Fatal(err)
	}

	env := minijinja.NewEnvironment()
	env.SetDebug(true)

	if err := env.AddTemplate("demo.txt", string(contents)); err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("demo.txt")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(map[string]any{
		"name":       "Peter Lustig",
		"iterations": 1,
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "debug"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "demo.txt")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate debug example files")
}
