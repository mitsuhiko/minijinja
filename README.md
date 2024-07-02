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
└── minijinja v2.0.3 (minijinja)
    └── serde v1.0.144
```

You can play with MiniJinja online [in the browser playground](https://mitsuhiko.github.io/minijinja-playground/)
powered by a WASM build of MiniJinja.

**Goals:**

* [Well documented](https://docs.rs/minijinja), compact API
* Minimal dependencies, reasonable compile times and [decent runtime performance](https://github.com/mitsuhiko/minijinja/tree/main/benchmarks#comparison-results)
* [Stay close as possible](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md) to Jinja2
* Support for [expression evaluation](https://docs.rs/minijinja/latest/minijinja/struct.Expression.html) which
  allows the use [as a DSL](https://github.com/mitsuhiko/minijinja/tree/main/examples/dsl)
* Support for all [`serde`](https://serde.rs) compatible types
* [Well tested](https://github.com/mitsuhiko/minijinja/tree/main/minijinja/tests)
* Support for [dynamic runtime objects](https://docs.rs/minijinja/latest/minijinja/value/trait.Object.html) with methods and dynamic attributes
* [Descriptive errors](https://github.com/mitsuhiko/minijinja/tree/main/examples/error)
* [Compiles to WebAssembly](https://github.com/mitsuhiko/minijinja-playground/blob/main/src/lib.rs)
* [Works with Python](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-py)
* Comes with a handy [CLI](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli)

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

## Getting Help

If you are stuck with `MiniJinja`, have suggestions or need help, you can use the
[GitHub Discussions](https://github.com/mitsuhiko/minijinja/discussions).

## Upgrading from MiniJinja 1.x

There are two major versions of MiniJinja both of which are currently maintained.  Most users should
upgrade to 2.x which has a much improved object system.  However if you have been using dynamic
objects in the past the upgrade might be quite involved.  For upgrade informations refer to
[UPDATING](UPDATING.md) which has a guide with examples of what the changes between the two engine
versions are.

To see examples and code from MiniJinja 1.x, you can browse the [minijinja-1.x branch](https://github.com/mitsuhiko/minijinja/tree/minijinja-1.x).

## Related Crates

* [minijinja-autoreload](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-autoreload): provides
  auto reloading functionality of environments
* [minijinja-embed](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-embed): provides
  utilities for embedding templates in a binary
* [minijinja-contrib](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-contrib): provides
  additional utilities too specific for the core
* [minijinja-py](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-py): makes MiniJinja
  available to Python
* [minijinja-cli](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli): a command line utility.
* [minijinja-cabi](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cabi): a C binding to MiniJinja.

## Similar Projects

These are related template engines for Rust:

* [Askama](https://crates.io/crates/askama): Jinja inspired, type-safe, requires template
  precompilation. Has significant divergence from Jinja syntax in parts.
* [Tera](https://crates.io/crates/tera): Jinja inspired, dynamic, has divergences from Jinja.
* [TinyTemplate](https://crates.io/crates/tinytemplate): minimal footprint template engine
  with syntax that takes lose inspiration from Jinja and handlebars.
* [Liquid](https://crates.io/crates/liquid): an implementation of Liquid templates for Rust.
  Liquid was inspired by Django from which Jinja took it's inspiration.

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Documentation](https://docs.rs/minijinja/)
- [Discussions](https://github.com/mitsuhiko/minijinja/discussions)
- [Examples](https://github.com/mitsuhiko/minijinja/tree/main/examples)
- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- [MiniJinja Playground](https://mitsuhiko.github.io/minijinja-playground/)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
