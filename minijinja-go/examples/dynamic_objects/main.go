// Example: Dynamic objects
//
// This example demonstrates the dynamic object capabilities of minijinja-go,
// porting the Rust dynamic-objects example.
package main

import (
	"fmt"
	"iter"
	"log"
	"sync/atomic"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// -----------------------------------------------------------------------------
// Cycler - A callable object with state
// -----------------------------------------------------------------------------

// Cycler cycles through a list of values each time it's called.
type Cycler struct {
	values []value.Value
	idx    atomic.Int64
}

func (c *Cycler) GetAttr(name string) value.Value {
	return value.Undefined()
}

// ObjectCall makes Cycler callable - cycler() returns the next value
func (c *Cycler) ObjectCall(state value.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if len(args) > 0 {
		return value.Undefined(), fmt.Errorf("cycler takes no arguments")
	}
	idx := c.idx.Add(1) - 1
	return c.values[int(idx)%len(c.values)], nil
}

// makeCycler is a function that creates a new Cycler from arguments
func makeCycler(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	// If a single array is passed, use its contents
	values := args
	if len(args) == 1 {
		if items := args[0].Iter(); items != nil {
			values = items
		}
	}
	return value.FromObject(&Cycler{
		values: values,
	}), nil
}

// -----------------------------------------------------------------------------
// Magic - An object with method calls
// -----------------------------------------------------------------------------

// Magic is an object that supports method calls like magic.make_class("ul")
type Magic struct{}

func (m *Magic) GetAttr(name string) value.Value {
	return value.Undefined()
}

// CallMethod handles method calls on the Magic object
func (m *Magic) CallMethod(state value.State, name string, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if name == "make_class" {
		if len(args) != 1 {
			return value.Undefined(), fmt.Errorf("make_class takes 1 argument")
		}
		tag, ok := args[0].AsString()
		if !ok {
			return value.Undefined(), fmt.Errorf("make_class argument must be string")
		}
		return value.FromString("magic-" + tag), nil
	}
	return value.Undefined(), value.ErrUnknownMethod
}

// -----------------------------------------------------------------------------
// SimpleDynamicSeq - A custom sequence
// -----------------------------------------------------------------------------

// SimpleDynamicSeq is an object that behaves like a sequence
type SimpleDynamicSeq struct {
	chars [4]rune
}

func (s *SimpleDynamicSeq) GetAttr(name string) value.Value {
	return value.Undefined()
}

func (s *SimpleDynamicSeq) ObjectRepr() value.ObjectRepr {
	return value.ObjectReprSeq
}

func (s *SimpleDynamicSeq) SeqLen() int {
	return len(s.chars)
}

func (s *SimpleDynamicSeq) SeqItem(index int) value.Value {
	if index >= 0 && index < len(s.chars) {
		return value.FromString(string(s.chars[index]))
	}
	return value.Undefined()
}

// -----------------------------------------------------------------------------
// Element - An object with a projected map attribute
// -----------------------------------------------------------------------------

// Element represents an HTML element with tag and attributes
type Element struct {
	Tag   string
	Attrs map[string]string
}

func (e *Element) GetAttr(name string) value.Value {
	switch name {
	case "tag":
		return value.FromString(e.Tag)
	case "attrs":
		// Use MakeObjectMap to create a map-like value that projects onto e.Attrs
		return value.MakeObjectMap(
			func() iter.Seq[value.Value] {
				return func(yield func(value.Value) bool) {
					for k := range e.Attrs {
						if !yield(value.FromString(k)) {
							return
						}
					}
				}
			},
			func(key value.Value) value.Value {
				if s, ok := key.AsString(); ok {
					if v, exists := e.Attrs[s]; exists {
						return value.FromString(v)
					}
				}
				return value.Undefined()
			},
		)
	}
	return value.Undefined()
}

// Keys returns the known keys for this object (for iteration/debug)
func (e *Element) Keys() []string {
	return []string{"tag", "attrs"}
}

// -----------------------------------------------------------------------------
// Main
// -----------------------------------------------------------------------------

func main() {
	env := minijinja.NewEnvironment()

	// Register the cycler function
	env.AddFunction("cycler", makeCycler)

	// Register global objects
	env.AddGlobal("magic", value.FromObject(&Magic{}))
	env.AddGlobal("seq", value.FromObject(&SimpleDynamicSeq{chars: [4]rune{'a', 'b', 'c', 'd'}}))
	env.AddGlobal("a_element", value.FromObject(&Element{
		Tag: "a",
		Attrs: map[string]string{
			"id":    "link-1",
			"class": "links",
		},
	}))

	// Create a lazy iterable using MakeIterable
	env.AddGlobal("real_iter", value.MakeIterable(func() iter.Seq[value.Value] {
		return func(yield func(value.Value) bool) {
			for i := 0; i < 10; i++ {
				if !yield(value.FromInt(int64(i))) {
					return
				}
			}
			for i := 20; i < 30; i++ {
				if !yield(value.FromInt(int64(i))) {
					return
				}
			}
		}
	}))

	// Template that uses all these features
	template := `{%- with next_class = cycler(["odd", "even"]) %}
  <ul class="{{ magic.make_class("ul") }}">
  {%- for char in seq %}
    <li class={{ next_class() }}>{{ char }}</li>
  {%- endfor %}
  </ul>
{%- endwith %}

{% for item in real_iter %}
  - {{ item }} ({{ loop.index }} from {{ loop.length|default("?") }})
{%- endfor %}

A element tag: {{ a_element.tag }}
A element attrs: {{ a_element.attrs }}
`

	err := env.AddTemplate("template.html", template)
	if err != nil {
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
