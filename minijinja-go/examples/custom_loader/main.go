// Example: custom-loader
//
// This example demonstrates using a custom template loader that validates
// template paths before reading from disk.
package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

func main() {
	templateDir, err := findTemplateDir()
	if err != nil {
		log.Fatal(err)
	}

	env := minijinja.NewEnvironment()
	env.SetLoader(func(name string) (string, error) {
		path := templateDir
		for _, piece := range strings.Split(name, "/") {
			if piece == "." || piece == ".." || strings.Contains(piece, "\\") {
				return "", fmt.Errorf("invalid template name: %s", name)
			}
			path = filepath.Join(path, piece)
		}

		contents, err := os.ReadFile(path)
		if err != nil {
			return "", err
		}
		return string(contents), nil
	})

	tmpl, err := env.GetTemplate("hello.txt")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(map[string]any{
		"name": "World",
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}

func findTemplateDir() (string, error) {
	candidates := []string{
		filepath.Join(".", "templates"),
		filepath.Join(".", "examples", "custom_loader", "templates"),
	}
	for _, candidate := range candidates {
		if info, err := os.Stat(candidate); err == nil && info.IsDir() {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate templates directory")
}
