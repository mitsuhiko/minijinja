<div align="center">
  <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
  <p><strong>MiniJinja: a powerful template engine for Rust with minimal dependencies</strong></p>

[![Build Status](https://github.com/mitsuhiko/minijinja/workflows/Tests/badge.svg?branch=main)](https://github.com/mitsuhiko/minijinja/actions?query=workflow%3ATests)
[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/d/minijinja.svg)](https://crates.io/crates/minijinja)
[![rustc 1.61.0](https://img.shields.io/badge/rust-1.61%2B-orange.svg)](https://img.shields.io/badge/rust-1.61%2B-orange.svg)
[![Documentation](https://docs.rs/minijinja/badge.svg)](https://docs.rs/minijinja)

</div>

MiniJinja is a powerful but minimal dependency template engine for Rust which
is based on the syntax and behavior of the
[Jinja2](https://jinja.palletsprojects.com/) template engine for Python.

It's implemented on top of `serde` and only has it as a single required
dependency. It supports [a range of features from Jinja2](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md)
including inheritance, filters and more.  The goal is that it should be possible
to use some templates in Rust programs without the fear of pulling in complex
dependencies for a small problem.  Additionally it tries not to re-invent
something but stay in line with prior art to leverage an already existing
ecosystem of editor integrations.

```
$ cargo tree
minimal v0.1.0 (examples/minimal)
└── minijinja v0.26.0 (minijinja)
    └── serde v1.0.144
```

**Goals:**

* [Well documented](https://docs.rs/minijinja), compact API
* Minimal dependencies, reasonable compile times and [decent runtime performance](https://github.com/mitsuhiko/minijinja/tree/main/benchmarks#comparison-results)
* [Stay close as possible](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md) to Jinja2
* Support for [expression evaluation](https://docs.rs/minijinja/latest/minijinja/struct.Expression.html)
* Support for all [`serde`](https://serde.rs) compatible types
* [Well tested](https://github.com/mitsuhiko/minijinja/tree/main/minijinja/tests)
* Support for [dynamic runtime objects](https://docs.rs/minijinja/latest/minijinja/value/trait.Object.html) with methods and dynamic attributes
* [Descriptive errors](https://github.com/mitsuhiko/minijinja/tree/main/examples/error)

## Example Template

```jinja
{% extends "layout.html" %}
{% block body %}
  <p>Hello {{ name }}!</p>
{% endblock %}
```

## API

```rust
use minijinja::{Environment, context};

fn main() {
    let mut env = Environment::new();
    env.add_template("hello.txt", "Hello {{ name }}!").unwrap();
    let template = env.get_template("hello.txt").unwrap();
    println!("{}", template.render(context! { name => "World" }).unwrap());
}
```

## Minimum Rust Version

MiniJinja's development version requires Rust 1.61 due to limitations with
HRTBs in older Rust versions.

MiniJinja 0.20 supports Rust versions down to 1.45.  It is possible to write
code that is compatible with both 0.20 and newer versions of MiniJinja which
should make it possible to defer the upgrade to later.

## Related Crates

* [minijinja-autoreload](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-autoreload): provides
* [minijinja-stack-ref](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-stack-ref): provides
  functionality to pass values from the stack

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Documentation](https://docs.rs/minijinja/)
- [Examples](https://github.com/mitsuhiko/minijinja/tree/main/examples)
- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
