// Example: none-as-undefined
//
// This example demonstrates using a custom formatter to treat None values
// as if they were undefined (rendering as empty strings and working with
// the default filter).
package main

import (
	"fmt"
	"log"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// Foo is an example struct with an optional field
type Foo struct {
	Bar *bool `json:"bar"`
}

// noneDefault is a filter similar to default() but also handles None values
func noneDefault(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if val.IsUndefined() || val.IsNone() {
		if len(args) > 0 {
			return args[0], nil
		}
		return value.FromString(""), nil
	}
	return val, nil
}

func main() {
	env := minijinja.NewEnvironment()

	// Replace the default filter with one that also handles None
	env.AddFilter("default", noneDefault)

	// Set a custom formatter that treats None as undefined
	env.SetFormatter(func(state *minijinja.State, val value.Value, escape func(string) string) string {
		// Treat None as undefined (render as empty string)
		if val.IsNone() {
			return ""
		}
		// Normal formatting with escaping
		s := val.String()
		if !val.IsSafe() {
			s = escape(s)
		}
		return s
	})

	err := env.AddTemplate("hello.txt",
		"A None attribute: {{ foo.bar }}\nWith default: {{ foo.bar|default(42) }}")
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("hello.txt")
	if err != nil {
		log.Fatal(err)
	}

	// Create a Foo with bar = nil (None)
	result, err := tmpl.Render(map[string]any{
		"foo": Foo{Bar: nil},
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
