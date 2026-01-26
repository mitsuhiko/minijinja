// Example: streaming
//
// This example demonstrates streaming template output using one-shot iterators.
// Items are generated lazily and the template output is written directly to
// a writer without buffering the entire result.
package main

import (
	"fmt"
	"log"
	"os"
	"time"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

const template = `
The stream we will iterate over: {{ stream }}
Results as they come in:

<ul>
{%- for item in stream %}
  <li>Item {{ item }}</li>
{%- endfor %}
</ul>

`

// generateItems creates a one-shot iterator that yields items with a delay
func generateItems() value.Value {
	return value.MakeOneShotIterator(func(yield func(value.Value) bool) {
		for i := 0; i < 20; i++ {
			// Simulate slow data generation
			time.Sleep(100 * time.Millisecond)
			fmt.Printf("[generating item %d]\n", i)
			if !yield(value.FromInt(int64(i))) {
				return // stop if consumer is done
			}
		}
	})
}

func main() {
	env := minijinja.NewEnvironment()

	err := env.AddTemplate("response.txt", template)
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("response.txt")
	if err != nil {
		log.Fatal(err)
	}

	// Render to stdout using streaming
	// Note: In Go, we buffer the output but the iterator is consumed lazily
	err = tmpl.RenderToWrite(map[string]any{
		"stream": generateItems(),
	}, os.Stdout)
	if err != nil {
		log.Fatal(err)
	}
}
