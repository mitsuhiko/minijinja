<div align="center">
  <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
  <p><strong>MiniJinja for Go: a powerful template engine for Go</strong></p>

[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Go Reference](https://pkg.go.dev/badge/github.com/mitsuhiko/minijinja/minijinja-go.svg)](https://pkg.go.dev/github.com/mitsuhiko/minijinja/minijinja-go)

</div>

MiniJinja for Go is a native Go port of the [MiniJinja](https://github.com/mitsuhiko/minijinja)
template engine. It's based on the syntax and behavior of the
[Jinja2](https://jinja.palletsprojects.com/) template engine for Python and aims
to provide the same functionality and compatibility as the original Rust implementation.

The goal is that it should be possible to use Jinja2 templates in Go programs
with high compatibility and without external dependencies. Additionally it tries
not to re-invent something but stay in line with prior art to leverage an already
existing ecosystem of editor integrations.

**Goals:**

* Native Go implementation with no CGO dependencies
* Stay as close as possible to [Jinja2](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md)
* Compatible with the original [MiniJinja](https://github.com/mitsuhiko/minijinja) for Rust
* Support for template inheritance, macros, and includes
* Rich set of built-in filters, tests, and functions
* Auto-escaping support for HTML templates
* Custom filters and functions

## AI Disclosure

This is alpha software which has been automatically ported from Rust go Go with the help of
Opus 4.5 and Codex 5.2.  The API might still change and there will be further validation about
some of the choices made.  It passes the reference tests from the Rust implementation with
minor adjustments however.

## Example

**Example Template:**

```jinja
{% extends "layout.html" %}
{% block body %}
  <p>Hello {{ name }}!</p>
{% endblock %}
```

**Invoking from Go:**

```go
package main

import (
    "fmt"
    "log"

    minijinja "github.com/mitsuhiko/minijinja/minijinja-go"
)

func main() {
    env := minijinja.NewEnvironment()
    env.AddTemplate("hello.txt", "Hello {{ name }}!")

    tmpl, err := env.GetTemplate("hello.txt")
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

## Installation

```bash
go get github.com/mitsuhiko/minijinja/minijinja-go
```

## Template Syntax

MiniJinja for Go supports the full Jinja2 template syntax:

* **Variable expressions:** `{{ name }}`
* **Comments:** `{# comment #}`
* **Control structures:** `{% if %}`, `{% for %}`, `{% set %}`, `{% with %}`
* **Template inheritance:** `{% extends %}`, `{% block %}`, `{{ super() }}`
* **Macros:** `{% macro %}` / `{% call %}`
* **Template inclusion:** `{% include %}`
* **Import statements:** `{% import %}`, `{% from ... import %}`
* **Filter blocks:** `{% filter %}`
* **Auto-escaping:** `{% autoescape %}`
* **Loop controls:** `{% break %}`, `{% continue %}`
* **Recursive loops:** `{% for ... recursive %}`

## Built-in Features

### Filters (50+)

* **String:** `upper`, `lower`, `capitalize`, `title`, `trim`, `replace`, `safe`, `escape`
* **List:** `first`, `last`, `length`, `reverse`, `sort`, `join`, `unique`, `batch`, `slice`
* **Math:** `abs`, `int`, `float`, `round`, `sum`, `min`, `max`
* **Dict:** `items`, `keys`, `values`, `dictsort`
* **Serialization:** `tojson`
* **URL:** `urlencode`
* **Formatting:** `indent`, `pprint`
* **Selection:** `map`, `select`, `reject`, `selectattr`, `rejectattr`

### Tests

* `defined`, `undefined`, `none`
* `true`, `false`
* `odd`, `even`, `divisibleby`
* `eq`, `ne`, `lt`, `le`, `gt`, `ge`
* `string`, `number`, `sequence`, `mapping`, `iterable`
* `startingwith`, `endingwith`, `containing`
* `in`

### Functions

* `range()` - generate sequences
* `dict()` - create dictionaries
* `namespace()` - create mutable namespaces
* `cycler()` - cycle through values
* `joiner()` - join with separators
* `debug()` - debug output
* `lipsum()` - lorem ipsum text

## Template Inheritance

MiniJinja for Go supports full template inheritance with `extends`, `block`, and `super()`:

```jinja
{# base.html #}
<html>
<head><title>{% block title %}Default{% endblock %}</title></head>
<body>
{% block content %}{% endblock %}
</body>
</html>
```

```jinja
{# child.html #}
{% extends "base.html" %}
{% block title %}My Page{% endblock %}
{% block content %}
<h1>Welcome!</h1>
<p>This is my page content.</p>
{% endblock %}
```

Use `{{ super() }}` to include the parent block's content:

```jinja
{% block content %}
{{ super() }}
<p>Additional content</p>
{% endblock %}
```

## Custom Filters and Functions

### Custom Filters

```go
env := minijinja.NewEnvironment()
env.AddFilter("double", func(state *minijinja.State, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
    if i, ok := val.AsInt(); ok {
        return value.FromInt(i * 2), nil
    }
    return val, nil
})
```

### Custom Functions

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

## Auto-Escaping

MiniJinja for Go automatically escapes HTML in templates with `.html`, `.htm`, or `.xml` extensions:

```go
env := minijinja.NewEnvironment()
tmpl, _ := env.TemplateFromNamedString("page.html", "{{ content }}")
result, _ := tmpl.Render(map[string]any{
    "content": "<script>alert('xss')</script>",
})
// Result: &lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;
```

Use the `safe` filter to mark content as safe:

```jinja
{{ content|safe }}
```

## Getting Help

If you are stuck with MiniJinja, have suggestions or need help, you can use the
[GitHub Discussions](https://github.com/mitsuhiko/minijinja/discussions).

## License and Links

* [Documentation](https://pkg.go.dev/github.com/mitsuhiko/minijinja/minijinja-go)
* [Discussions](https://github.com/mitsuhiko/minijinja/discussions)
* [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
* License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
