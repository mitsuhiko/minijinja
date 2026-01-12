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

### Implemented

- **Template Syntax**
  - Variable expressions: `{{ name }}`
  - Comments: `{# comment #}`
  - Control structures: `{% if %}`, `{% for %}`, `{% set %}`, `{% with %}`
  - Macros: `{% macro %}` / `{% call %}`
  - Template inclusion: `{% include %}`
  - Filter blocks: `{% filter %}`
  - Auto-escaping: `{% autoescape %}`

- **Expressions**
  - Literals: strings, numbers, booleans, lists, dicts
  - Arithmetic: `+`, `-`, `*`, `/`, `//`, `%`, `**`
  - Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`
  - Logical: `and`, `or`, `not`
  - Membership: `in`
  - Concatenation: `~`
  - Conditional: `x if cond else y`

- **Filters** (50+)
  - String: `upper`, `lower`, `capitalize`, `title`, `trim`, `replace`
  - List: `first`, `last`, `length`, `reverse`, `sort`, `join`, `unique`
  - Math: `abs`, `int`, `float`, `round`, `sum`, `min`, `max`
  - Dict: `items`, `keys`, `values`, `dictsort`
  - And many more...

- **Tests**
  - `defined`, `undefined`, `none`
  - `true`, `false`
  - `odd`, `even`, `divisibleby`
  - `eq`, `ne`, `lt`, `le`, `gt`, `ge`
  - `string`, `number`, `sequence`, `mapping`, `iterable`
  - `startingwith`, `endingwith`, `containing`

- **Functions**
  - `range()`, `dict()`, `namespace()`, `debug()`, `lipsum()`

### Not Yet Implemented

- Template inheritance (`{% extends %}`, `{% block %}`)
- Import statements (`{% import %}`, `{% from ... import %}`)
- Custom syntax delimiters (configurable but not tested)
- Some advanced filters

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

```go
tmpl, _ := env.TemplateFromNamedString("page.html", "{{ content|safe }}")
```

## Custom Filters

```go
env := minijinja.NewEnvironment()
env.AddFilter("double", func(state *minijinja.State, val minijinja.Value, args []minijinja.Value, kwargs map[string]minijinja.Value) (minijinja.Value, error) {
    if i, ok := val.AsInt(); ok {
        return minijinja.FromInt(i * 2), nil
    }
    return val, nil
})
```

## Custom Functions

```go
env := minijinja.NewEnvironment()
env.AddFunction("now", func(state *minijinja.State, args []minijinja.Value, kwargs map[string]minijinja.Value) (minijinja.Value, error) {
    return minijinja.FromString(time.Now().Format(time.RFC3339)), nil
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
