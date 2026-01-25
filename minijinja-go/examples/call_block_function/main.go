// Example: call-block-function
//
// This example demonstrates how to call back into a block from a custom
// function using the `caller` callable.
package main

import (
	"fmt"
	"log"
	"strings"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

const templateSource = `{%- macro run_loop(num) -%}
{{ custom_loop(num, caller=caller) }}
{%- endmacro %}
Before the loop
{%- call(it) run_loop(5) %}
  Iteration {{ it }}!
{%- endcall %}
After the loop`

func customLoop(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if len(args) != 1 {
		return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "custom_loop expects a number")
	}

	num, ok := args[0].AsInt()
	if !ok {
		return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "custom_loop expects a number")
	}

	callerValue, ok := kwargs["caller"]
	if !ok {
		return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "caller must be callable")
	}

	caller, ok := callerValue.AsCallable()
	if !ok {
		return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "caller must be callable")
	}

	var builder strings.Builder
	for i := int64(0); i < num; i++ {
		rendered, err := caller.Call(state, []value.Value{value.FromInt(i + 1)}, nil)
		if err != nil {
			return value.Undefined(), err
		}

		text, ok := rendered.AsString()
		if !ok {
			return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "caller did not return a string")
		}
		builder.WriteString(text)
	}

	return value.FromString(builder.String()), nil
}

func main() {
	env := minijinja.NewEnvironment()
	env.AddFunction("custom_loop", customLoop)

	tmpl, err := env.TemplateFromString(templateSource)
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
