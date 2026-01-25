// Example: load-lazy
//
// This example demonstrates lazy attribute loading on a custom object.
package main

import (
	"encoding/json"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

type Site struct {
	baseDir string
	cache   map[string]value.Value
	mu      sync.Mutex
}

func (s *Site) GetAttr(name string) value.Value {
	s.mu.Lock()
	defer s.mu.Unlock()

	if val, ok := s.cache[name]; ok {
		return val
	}

	val, ok := loadJSON(s.baseDir, name)
	if !ok {
		return value.Undefined()
	}

	s.cache[name] = val
	return val
}

func loadJSON(baseDir, name string) (value.Value, bool) {
	path := baseDir
	for _, segment := range strings.Split(name, "/") {
		if strings.HasPrefix(segment, ".") || strings.Contains(segment, "\\") {
			return value.Undefined(), false
		}
		path = filepath.Join(path, segment)
	}
	path += ".json"

	contents, err := os.ReadFile(path)
	if err != nil {
		return value.Undefined(), false
	}

	var parsed any
	if err := json.Unmarshal(contents, &parsed); err != nil {
		return value.Undefined(), false
	}

	return value.FromAny(parsed), true
}

func main() {
	baseDir, err := findExampleDir()
	if err != nil {
		log.Fatal(err)
	}

	contents, err := os.ReadFile(filepath.Join(baseDir, "template.html"))
	if err != nil {
		log.Fatal(err)
	}

	env := minijinja.NewEnvironment()
	env.AddGlobal("site", value.FromObject(&Site{baseDir: baseDir, cache: make(map[string]value.Value)}))
	if err := env.AddTemplate("template.html", string(contents)); err != nil {
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

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "load_lazy"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "template.html")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate load-lazy example files")
}
