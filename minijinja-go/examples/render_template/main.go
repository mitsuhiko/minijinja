// Example: Render a template from disk with a JSON context.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

func main() {
	var contextPath string
	var templatePath string

	flag.StringVar(&contextPath, "context", "", "path to a JSON file with the context")
	flag.StringVar(&contextPath, "c", "", "path to a JSON file with the context (shorthand)")
	flag.StringVar(&templatePath, "template", "", "path to a template file to render")
	flag.StringVar(&templatePath, "t", "", "path to a template file to render (shorthand)")
	flag.Parse()

	if contextPath == "" && templatePath == "" {
		baseDir, err := findExampleDir()
		if err != nil {
			log.Fatal(err)
		}
		contextPath = filepath.Join(baseDir, "users.json")
		templatePath = filepath.Join(baseDir, "users.html")
	} else if contextPath == "" || templatePath == "" {
		flag.Usage()
		log.Fatal("context and template paths are required")
	}

	env := minijinja.NewEnvironment()

	source, err := os.ReadFile(templatePath)
	if err != nil {
		log.Fatalf("failed to read template: %v", err)
	}
	name := filepath.Base(templatePath)
	if err := env.AddTemplate(name, string(source)); err != nil {
		log.Fatalf("failed to add template: %v", err)
	}

	data, err := os.ReadFile(contextPath)
	if err != nil {
		log.Fatalf("failed to read context: %v", err)
	}
	var ctx any
	if err := json.Unmarshal(data, &ctx); err != nil {
		log.Fatalf("failed to parse JSON context: %v", err)
	}

	tmpl, err := env.GetTemplate(name)
	if err != nil {
		log.Fatalf("failed to get template: %v", err)
	}

	result, err := tmpl.Render(ctx)
	if err != nil {
		log.Fatalf("failed to render template: %v", err)
	}

	fmt.Println(result)
}

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "render_template"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "users.json")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate render-template example files")
}
