<div align="center">
  <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
  <p><strong>MiniJinja for Go: a powerful template engine for Go</strong></p>

[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Go Reference](https://pkg.go.dev/badge/github.com/mitsuhiko/minijinja/minijinja-go/v2.svg)](https://pkg.go.dev/github.com/mitsuhiko/minijinja/minijinja-go/v2)

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

This is alpha software which has been automatically ported from Rust go Go with
the help of Opus 4.5 and Codex 5.2.  The API might still change and there will
be further validation about some of the choices made.  It passes the reference
tests from the Rust implementation with minor adjustments however.

The implementation intentionally diverges from Rust to make sense in the Go.  One
significant departure has been that unlike the Rust implementation which uses a
bytecode interpreter VM, this is just an AST walker.

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

    minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
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

## More Examples

- `examples/error_stacktrace`: demonstrates debug stacktraces with template snippets, locals, and chained include errors.

## Installation

```bash
go get github.com/mitsuhiko/minijinja/minijinja-go/v2
```

Documentation: https://pkg.go.dev/github.com/mitsuhiko/minijinja/minijinja-go/v2

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

In addition to the filters supported out of the box, you can register your own ones:

### Custom Filters

```go
env := minijinja.NewEnvironment()
env.AddFilter("double", func(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
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

## Context Support

MiniJinja for Go supports Go's `context.Context` for cancellation, timeouts, and passing request-scoped values:

```go
// Use RenderCtx to pass a context
ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
defer cancel()
result, err := tmpl.RenderCtx(ctx, data)

// Access the context in custom filters/functions via State.Context()
env.AddFunction("request_id", func(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
    ctx := state.Context()
    if id, ok := ctx.Value(requestIDKey{}).(string); ok {
        return value.FromString(id), nil
    }
    return value.FromString("unknown"), nil
})
```

## Auto-Escaping

MiniJinja for Go automatically escapes HTML in templates with `.html`, `.htm`, or `.xml` extensions, and
serializes values as JSON for `.json`, `.json5`, `.js`, `.yaml`, and `.yml` files. The `.j2`, `.jinja`, and
`.jinja2` suffixes are ignored when determining the default auto-escape mode:

```go
env := minijinja.NewEnvironment()
tmpl, _ := env.TemplateFromNamedString("page.html", "{{ content }}")
result, _ := tmpl.Render(map[string]any{
    "content": "<script>alert('xss')</script>",
})
// Result: &lt;script&gt;alert(&#x27;xss&#x27;)&lt;&#x2f;script&gt;
```

Use the `safe` filter to mark content as safe:

```jinja
{{ content|safe }}
```

## Getting Help

If you are stuck with MiniJinja, have suggestions or need help, you can use the
[GitHub Discussions](https://github.com/mitsuhiko/minijinja/discussions).

## License and Links

* [Documentation](https://pkg.go.dev/github.com/mitsuhiko/minijinja/minijinja-go/v2)
* [Discussions](https://github.com/mitsuhiko/minijinja/discussions)
* [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
* License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
