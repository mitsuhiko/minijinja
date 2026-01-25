// Example: dynamic-context
//
// This example demonstrates using a dynamic map-like object as the root context.
package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

type DynamicContext struct{}

func (d *DynamicContext) GetAttr(name string) value.Value {
	switch name {
	case "pid":
		return value.FromInt(int64(os.Getpid()))
	case "cwd":
		cwd, err := os.Getwd()
		if err != nil {
			return value.Undefined()
		}
		return value.FromString(cwd)
	case "env":
		vars := make(map[string]string)
		for _, entry := range os.Environ() {
			parts := strings.SplitN(entry, "=", 2)
			if len(parts) != 2 {
				continue
			}
			key := parts[0]
			if strings.HasPrefix(key, "CARGO_") || strings.HasPrefix(key, "RUST_") {
				vars[key] = parts[1]
			}
		}
		return value.FromAny(vars)
	default:
		return value.Undefined()
	}
}

func (d *DynamicContext) ObjectRepr() value.ObjectRepr {
	return value.ObjectReprMap
}

func (d *DynamicContext) Keys() []string {
	return []string{"pid", "cwd", "env"}
}

func main() {
	baseDir, err := findExampleDir()
	if err != nil {
		log.Fatal(err)
	}

	contents, err := os.ReadFile(filepath.Join(baseDir, "template.txt"))
	if err != nil {
		log.Fatal(err)
	}

	env := minijinja.NewEnvironment()
	if err := env.AddTemplate("template.txt", string(contents)); err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("template.txt")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(value.FromObject(&DynamicContext{}))
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "dynamic_context"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "template.txt")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate dynamic-context example files")
}
