// Example: value-tracking
//
// This example demonstrates how to track variable lookups during rendering.
package main

import (
	"fmt"
	"log"
	"sort"
	"sync"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

type trackedContext struct {
	enclosed value.Value
	resolved map[string]struct{}
	mu       sync.Mutex
}

func (t *trackedContext) GetAttr(name string) value.Value {
	t.mu.Lock()
	t.resolved[name] = struct{}{}
	t.mu.Unlock()

	v := t.enclosed.GetAttr(name)
	if v.IsUndefined() {
		return value.Undefined()
	}
	return v
}

func (t *trackedContext) ObjectRepr() value.ObjectRepr {
	return value.ObjectReprMap
}

func (t *trackedContext) Keys() []string {
	if m, ok := t.enclosed.AsMap(); ok {
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		return keys
	}

	if obj, ok := t.enclosed.AsObject(); ok {
		if m, ok := obj.(value.MapObject); ok {
			return m.Keys()
		}
	}

	return nil
}

func trackContext(ctx value.Value) (value.Value, map[string]struct{}) {
	resolved := make(map[string]struct{})
	return value.FromObject(&trackedContext{
		enclosed: ctx,
		resolved: resolved,
	}), resolved
}

func snapshotResolved(resolved map[string]struct{}) []string {
	keys := make([]string, 0, len(resolved))
	for name := range resolved {
		keys = append(keys, name)
	}
	sort.Strings(keys)
	return keys
}

const template = `
{%- set locally_set = 'a-value' -%}
name={{ name }}
undefined_value={{ undefined_value }}
global={{ global }}
locally_set={{ locally_set }}
`

func main() {
	env := minijinja.NewEnvironment()
	env.AddGlobal("global", value.FromBool(true))

	tmpl, err := env.TemplateFromString(template)
	if err != nil {
		log.Fatal(err)
	}

	ctx, resolved := trackContext(value.FromAny(map[string]any{
		"name":   "John",
		"unused": 42,
	}))

	result, err := tmpl.Render(ctx)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(result)
	fmt.Printf("resolved: %v\n", snapshotResolved(resolved))
}
