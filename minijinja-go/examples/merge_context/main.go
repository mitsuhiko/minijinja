// Example: merge-context
//
// This example demonstrates how to merge context values when rendering templates.
// In Rust's MiniJinja this is done with `context! { ..ctx }` syntax. In Go we
// can use minijinja.MergeMaps to create a lazy merged context that combines
// multiple sources, including dynamic objects.
package main

import (
	"fmt"
	"log"
	"time"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// runtimeContext provides dynamic values that are computed on access.
type runtimeContext struct {
	region string
}

func (r *runtimeContext) ObjectRepr() value.ObjectRepr {
	return value.ObjectReprMap
}

func (r *runtimeContext) Keys() []string {
	return []string{"now", "region"}
}

func (r *runtimeContext) GetAttr(name string) value.Value {
	switch name {
	case "now":
		return value.FromString(time.Now().Format(time.RFC3339))
	case "region":
		return value.FromString(r.region)
	}
	return value.Undefined()
}

func main() {
	env := minijinja.NewEnvironment()

	err := env.AddTemplate("template.txt",
		"User: {{ user }}\nTime: {{ now }}\nRegion: {{ region }}\nPlan: {{ plan }}")
	if err != nil {
		log.Fatal(err)
	}

	runtime := &runtimeContext{region: "fra"}
	defaults := map[string]any{
		"plan":   "free",
		"region": "iad",
	}
	user := map[string]any{
		"user":   "Jane Doe",
		"region": "lhr",
	}

	ctx := minijinja.MergeMaps(runtime, defaults, user)

	tmpl, err := env.GetTemplate("template.txt")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(ctx)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
