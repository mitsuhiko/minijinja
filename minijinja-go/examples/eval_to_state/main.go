// Example: eval-to-state
//
// This example demonstrates using EvalToState to evaluate a template and then
// access its blocks, macros, and exports programmatically.
package main

import (
	"fmt"
	"log"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

const layoutHTML = `<!doctype html>
<title>{% block title %}{% endblock %} | My Site</title>
<nav>
  <ul>
    <li><a href="{{ site_url }}/index.html">Index</a></li>
    <li><a href="{{ site_url }}/about.html">About</a></li>
  </ul>
</nav>
{%- set global_variable = 42 %}
<div class="content">
  {% block body %}{% endblock %}
</div>`

const indexHTML = `{% extends "layout.html" %}
{% macro utility() %}Global var is {{ global_variable }}{% endmacro %}
{% block title %}Index{% endblock %}
{% block body %}
Hello from index.html
{{ utility() }}
{% endblock %}`

func main() {
	env := minijinja.NewEnvironment()

	err := env.AddTemplate("layout.html", layoutHTML)
	if err != nil {
		log.Fatal(err)
	}

	err = env.AddTemplate("index.html", indexHTML)
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("index.html")
	if err != nil {
		log.Fatal(err)
	}

	// Evaluate the template to get its state
	state, err := tmpl.EvalToState(map[string]any{
		"site_url": "http://example.com",
	})
	if err != nil {
		log.Fatal(err)
	}

	// Render specific blocks
	title, err := state.RenderBlock("title")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("Block 'title': %q\n", title)

	body, err := state.RenderBlock("body")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("Block 'body': %q\n", body)

	// Call a macro programmatically
	utilityResult, err := state.CallMacro("utility")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("Macro 'utility': %q\n", utilityResult.String())

	// Look up variables
	globalVar := state.Lookup("global_variable")
	fmt.Printf("Variable 'global_variable': %v\n", globalVar)

	// Get exports
	exports := state.Exports()
	fmt.Printf("Exports: %v\n", exports)

	// Template metadata
	fmt.Printf("Template name: %q\n", state.Name())
	fmt.Printf("Undefined behavior: %v\n", state.UndefinedBehavior())

	// Look up a built-in function
	rangeFn := state.Lookup("range")
	fmt.Printf("Range function resolved: %v\n", rangeFn)

	// Call the range function
	if callable, ok := rangeFn.AsCallable(); ok {
		result, err := callable.Call(state, []value.Value{value.FromInt(5)}, nil)
		if err != nil {
			log.Fatal(err)
		}
		fmt.Printf("Range function invoked: %v\n", result)
	}
}
