# MiniJinja-Go

A Go port of the [MiniJinja](https://github.com/mitsuhiko/minijinja) template engine.

## Overview

MiniJinja-Go is a Jinja2-compatible template engine for Go. It aims to provide
the same functionality and compatibility as the original Rust implementation.

## Installation

```bash
go get github.com/mitsuhiko/minijinja/minijinja-go
```

## Usage

```go
package main

import (
    "fmt"
    "log"

    minijinja "github.com/mitsuhiko/minijinja/minijinja-go"
)

func main() {
    // Create a new environment
    env := minijinja.NewEnvironment()

    // Add a template
    err := env.AddTemplate("hello.html", "Hello {{ name }}!")
    if err != nil {
        log.Fatal(err)
    }

    // Get and render the template
    tmpl, err := env.GetTemplate("hello.html")
    if err != nil {
        log.Fatal(err)
    }

    result, err := tmpl.Render(map[string]any{"name": "World"})
    if err != nil {
        log.Fatal(err)
    }

    fmt.Println(result) // Output: Hello World!
}
```

## Features

### Template Syntax

- Variable expressions: `{{ name }}`
- Comments: `{# comment #}`
- Control structures: `{% if %}`, `{% for %}`, `{% set %}`, `{% with %}`
- Template inheritance: `{% extends %}`, `{% block %}`, `{{ super() }}`
- Macros: `{% macro %}` / `{% call %}`
- Template inclusion: `{% include %}`
- Import statements: `{% import %}`, `{% from ... import %}`
- Filter blocks: `{% filter %}`
- Auto-escaping: `{% autoescape %}`
- Loop controls: `{% break %}`, `{% continue %}`
- Recursive loops: `{% for ... recursive %}`

### Expressions

- Literals: strings, numbers, booleans, lists, dicts
- Arithmetic: `+`, `-`, `*`, `/`, `//`, `%`, `**`
- Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Logical: `and`, `or`, `not`
- Membership: `in`
- Concatenation: `~`
- Conditional: `x if cond else y`
- Slicing: `seq[1:3]`, `seq[::2]`

### Filters (50+)

- String: `upper`, `lower`, `capitalize`, `title`, `trim`, `replace`, `safe`, `escape`
- List: `first`, `last`, `length`, `reverse`, `sort`, `join`, `unique`, `batch`, `slice`
- Math: `abs`, `int`, `float`, `round`, `sum`, `min`, `max`
- Dict: `items`, `keys`, `values`, `dictsort`
- Serialization: `tojson`
- URL: `urlencode`
- Formatting: `indent`, `pprint`
- Selection: `map`, `select`, `reject`, `selectattr`, `rejectattr`

### Tests

- `defined`, `undefined`, `none`
- `true`, `false`
- `odd`, `even`, `divisibleby`
- `eq`, `ne`, `lt`, `le`, `gt`, `ge`
- `string`, `number`, `sequence`, `mapping`, `iterable`
- `startingwith`, `endingwith`, `containing`
- `in`

### Functions

- `range()` - generate sequences
- `dict()` - create dictionaries
- `namespace()` - create mutable namespaces
- `cycler()` - cycle through values
- `joiner()` - join with separators
- `debug()` - debug output
- `lipsum()` - lorem ipsum text

## Template Inheritance

MiniJinja-Go supports template inheritance with `extends`, `block`, and `super()`:

```html
{# base.html #}
<html>
<head><title>{% block title %}Default{% endblock %}</title></head>
<body>
{% block content %}{% endblock %}
</body>
</html>
```

```html
{# child.html #}
{% extends "base.html" %}
{% block title %}My Page{% endblock %}
{% block content %}
<h1>Welcome!</h1>
<p>This is my page content.</p>
{% endblock %}
```

Use `{{ super() }}` to include the parent block's content:

```html
{% block content %}
{{ super() }}
<p>Additional content</p>
{% endblock %}
```

## Macros and Imports

Define reusable macros:

```html
{# forms.html #}
{% macro input(name, type="text") %}
<input type="{{ type }}" name="{{ name }}">
{% endmacro %}

{% macro button(text) %}
<button>{{ text }}</button>
{% endmacro %}
```

Import and use them:

```html
{# Full import #}
{% import "forms.html" as forms %}
{{ forms.input("username") }}
{{ forms.button("Submit") }}

{# Selective import #}
{% from "forms.html" import input, button %}
{{ input("email", type="email") }}
{{ button("Send") }}

{# Import with alias #}
{% from "forms.html" import input as text_input %}
{{ text_input("name") }}
```

## Auto-Escaping

MiniJinja-Go automatically escapes HTML in templates with `.html`, `.htm`, or `.xml` extensions:

```go
env := minijinja.NewEnvironment()
tmpl, _ := env.TemplateFromNamedString("page.html", "{{ content }}")
result, _ := tmpl.Render(map[string]any{
    "content": "<script>alert('xss')</script>",
})
// Result: &lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;
```

Use the `safe` filter to mark content as safe:

```html
{{ content|safe }}
```

## Callable Objects

### Cycler

Cycle through a list of values:

```html
{% set c = cycler("odd", "even") %}
{% for item in items %}
<tr class="{{ c.next() }}">{{ item }}</tr>
{% endfor %}
```

### Joiner

Add separators between items:

```html
{% set j = joiner(", ") %}
{% for item in items %}{{ j() }}{{ item }}{% endfor %}
```

### Namespace

Create mutable namespaces for use across scopes:

```html
{% set ns = namespace(count=0) %}
{% for item in items %}
  {% set ns.count = ns.count + 1 %}
{% endfor %}
Total: {{ ns.count }}
```

## Custom Filters

```go
env := minijinja.NewEnvironment()
env.AddFilter("double", func(state *minijinja.State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
    if i, ok := val.AsInt(); ok {
        return value.FromInt(i * 2), nil
    }
    return val, nil
})
```

## Custom Functions

```go
env := minijinja.NewEnvironment()
env.AddFunction("now", func(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
    return value.FromString(time.Now().Format(time.RFC3339)), nil
})
```

## Template Loading

```go
env := minijinja.NewEnvironment()
env.SetLoader(func(name string) (string, error) {
    content, err := os.ReadFile(filepath.Join("templates", name))
    if err != nil {
        return "", err
    }
    return string(content), nil
})

tmpl, err := env.GetTemplate("index.html")
```

## License

Licensed under the Apache License 2.0.
