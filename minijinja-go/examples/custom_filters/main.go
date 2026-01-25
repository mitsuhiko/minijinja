// Example: Custom filters, tests, and functions
//
// This example shows how to extend MiniJinja with custom functionality.
package main

import (
	"fmt"
	"log"
	"strings"
	"time"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

func main() {
	env := minijinja.NewEnvironment()

	// =========================================
	// Custom Filters
	// =========================================

	// A simple filter that reverses a string
	env.AddFilter("reverse_str", func(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		s, ok := val.AsString()
		if !ok {
			return value.Undefined(), fmt.Errorf("reverse_str expects a string")
		}
		runes := []rune(s)
		for i, j := 0, len(runes)-1; i < j; i, j = i+1, j-1 {
			runes[i], runes[j] = runes[j], runes[i]
		}
		return value.FromString(string(runes)), nil
	})

	// A filter with arguments that wraps text in a tag
	env.AddFilter("wrap", func(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		s, ok := val.AsString()
		if !ok {
			return value.Undefined(), fmt.Errorf("wrap expects a string")
		}

		tag := "span"
		if len(args) > 0 {
			if t, ok := args[0].AsString(); ok {
				tag = t
			}
		}

		// Check for class kwarg
		class := ""
		if c, ok := kwargs["class"]; ok {
			if cs, ok := c.AsString(); ok {
				class = fmt.Sprintf(` class="%s"`, cs)
			}
		}

		result := fmt.Sprintf("<%s%s>%s</%s>", tag, class, s, tag)
		// Return as safe string since we're generating HTML
		return value.FromSafeString(result), nil
	})

	// =========================================
	// Custom Tests
	// =========================================

	// A test that checks if a string starts with a prefix
	env.AddTest("startswith", func(state minijinja.TestState, val value.Value, args []value.Value) (bool, error) {
		s, ok := val.AsString()
		if !ok {
			return false, nil
		}
		if len(args) == 0 {
			return false, fmt.Errorf("startswith requires a prefix argument")
		}
		prefix, ok := args[0].AsString()
		if !ok {
			return false, nil
		}
		return strings.HasPrefix(s, prefix), nil
	})

	// A test for checking if a number is in a range
	env.AddTest("between", func(state minijinja.TestState, val value.Value, args []value.Value) (bool, error) {
		n, ok := val.AsInt()
		if !ok {
			return false, nil
		}
		if len(args) < 2 {
			return false, fmt.Errorf("between requires min and max arguments")
		}
		min, _ := args[0].AsInt()
		max, _ := args[1].AsInt()
		return n >= min && n <= max, nil
	})

	// =========================================
	// Custom Functions
	// =========================================

	// A function that returns the current time
	env.AddFunction("now", func(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		format := time.RFC3339
		if len(args) > 0 {
			if f, ok := args[0].AsString(); ok {
				format = f
			}
		}
		return value.FromString(time.Now().Format(format)), nil
	})

	// A function that creates a greeting
	env.AddFunction("greet", func(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		name := "World"
		if len(args) > 0 {
			if n, ok := args[0].AsString(); ok {
				name = n
			}
		}

		// Check for style kwarg
		style := "formal"
		if s, ok := kwargs["style"]; ok {
			if sv, ok := s.AsString(); ok {
				style = sv
			}
		}

		var greeting string
		switch style {
		case "casual":
			greeting = fmt.Sprintf("Hey %s!", name)
		case "enthusiastic":
			greeting = fmt.Sprintf("Hello %s!!!", name)
		default:
			greeting = fmt.Sprintf("Hello, %s.", name)
		}
		return value.FromString(greeting), nil
	})

	// =========================================
	// Example usage
	// =========================================

	err := env.AddTemplate("demo.html", `
Custom Filters:
  reverse_str: {{ "hello"|reverse_str }}
  wrap: {{ "text"|wrap("div", class="highlight") }}

Custom Tests:
  "hello" startswith "hel": {{ "hello" is startswith("hel") }}
  "world" startswith "hel": {{ "world" is startswith("hel") }}
  5 between 1,10: {{ 5 is between(1, 10) }}
  15 between 1,10: {{ 15 is between(1, 10) }}

Custom Functions:
  greet(): {{ greet() }}
  greet("Alice"): {{ greet("Alice") }}
  greet("Bob", style="casual"): {{ greet("Bob", style="casual") }}
  greet("Carol", style="enthusiastic"): {{ greet("Carol", style="enthusiastic") }}
`)
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("demo.html")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
