// Example: Loading templates from the filesystem
//
// This example demonstrates how to set up a template loader that loads
// templates from a directory on disk.
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

	// Get the template directory (relative to this example)
	// In a real application, this might be a config value or flag
	templateDir := filepath.Join(baseDir, "templates")

	env := minijinja.NewEnvironment()

	// Set up a loader that reads templates from disk.
	// The loader is called when GetTemplate() is called for a template
	// that hasn't been loaded yet.
	env.SetLoader(func(name string) (string, error) {
		path := filepath.Join(templateDir, name)

		// Security: Ensure the resolved path is within the template directory
		absPath, err := filepath.Abs(path)
		if err != nil {
			return "", err
		}
		absDir, err := filepath.Abs(templateDir)
		if err != nil {
			return "", err
		}
		if !filepath.HasPrefix(absPath, absDir) {
			return "", fmt.Errorf("template path escapes template directory: %s", name)
		}

		content, err := os.ReadFile(absPath)
		if err != nil {
			return "", err
		}
		return string(content), nil
	})

	// Now templates are loaded on demand
	tmpl, err := env.GetTemplate("index.html")
	if err != nil {
		log.Fatalf("Failed to load template: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"title":   "Welcome",
		"message": "Hello from the file-based template!",
		"items":   []string{"one", "two", "three"},
	})
	if err != nil {
		log.Fatalf("Failed to render: %v", err)
	}

	fmt.Println(result)

	// The loader also supports template inheritance and includes.
	// Templates can reference other templates by name.
	tmpl2, err := env.GetTemplate("page.html")
	if err != nil {
		log.Fatalf("Failed to load template: %v", err)
	}

	result2, err := tmpl2.Render(map[string]any{
		"page_title": "About Us",
		"content":    "This is the about page.",
	})
	if err != nil {
		log.Fatalf("Failed to render: %v", err)
	}

	fmt.Println("\n--- Extended template ---")
	fmt.Println(result2)
}

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "loader"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "templates", "index.html")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate loader example templates")
}
