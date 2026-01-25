// Example: load-resource
//
// This example demonstrates loading JSON resources from a custom function.
package main

import (
	"encoding/json"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func main() {
	baseDir, err := findExampleDir()
	if err != nil {
		log.Fatal(err)
	}

	templateSource, err := os.ReadFile(filepath.Join(baseDir, "template.html"))
	if err != nil {
		log.Fatal(err)
	}

	env := minijinja.NewEnvironment()
	env.AddFunction("load_data", func(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		if len(args) != 1 {
			return value.Undefined(), minijinja.NewError(minijinja.ErrMissingArgument, "load_data expects a filename")
		}
		filename, ok := args[0].AsString()
		if !ok {
			return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "filename must be a string")
		}
		return loadJSONResource(baseDir, filename)
	})

	if err := env.AddTemplate("template.html", string(templateSource)); err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("template.html")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}

func loadJSONResource(baseDir, filename string) (value.Value, error) {
	path := baseDir
	for _, segment := range strings.Split(filename, "/") {
		if strings.HasPrefix(segment, ".") || strings.Contains(segment, "\\") {
			return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "bad filename")
		}
		path = filepath.Join(path, segment)
	}

	contents, err := os.ReadFile(path)
	if err != nil {
		return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "could not read JSON file")
	}

	var parsed any
	if err := json.Unmarshal(contents, &parsed); err != nil {
		return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "invalid JSON")
	}

	return value.FromAny(parsed), nil
}

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "load_resource"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "template.html")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate load-resource example files")
}
