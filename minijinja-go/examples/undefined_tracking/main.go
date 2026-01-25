// Example: undefined-tracking
//
// This example demonstrates how to track variables that are requested in a
// template but missing from the provided context.
package main

import (
	"fmt"
	"log"
	"sort"
	"sync"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

type trackedContext struct {
	enclosed  value.Value
	undefined map[string]struct{}
	mu        sync.Mutex
}

func (t *trackedContext) GetAttr(name string) value.Value {
	v := t.enclosed.GetAttr(name)
	if !v.IsUndefined() {
		return v
	}

	t.mu.Lock()
	t.undefined[name] = struct{}{}
	t.mu.Unlock()

	return value.Undefined()
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
	undefined := make(map[string]struct{})
	return value.FromObject(&trackedContext{
		enclosed:  ctx,
		undefined: undefined,
	}), undefined
}

func snapshotUndefined(ctx value.Value, undefined map[string]struct{}) []string {
	obj, ok := ctx.AsObject()
	if !ok {
		return nil
	}

	tracked, ok := obj.(*trackedContext)
	if !ok {
		return nil
	}

	tracked.mu.Lock()
	defer tracked.mu.Unlock()

	keys := make([]string, 0, len(undefined))
	for name := range undefined {
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

	ctx, undefined := trackContext(value.FromAny(map[string]any{
		"name":   "John",
		"unused": 42,
	}))

	result, err := tmpl.Render(ctx)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(result)

	state, err := tmpl.EvalToState(ctx)
	if err != nil {
		log.Fatal(err)
	}

	allUndefined := snapshotUndefined(ctx, undefined)
	fmt.Printf("not found in context: %v\n", allUndefined)

	completelyUndefined := make([]string, 0, len(allUndefined))
	for _, name := range allUndefined {
		if state.Lookup(name).IsUndefined() {
			completelyUndefined = append(completelyUndefined, name)
		}
	}
	fmt.Printf("completely undefined: %v\n", completelyUndefined)
}
