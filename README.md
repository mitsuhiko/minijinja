<div align="center">
  <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
  <p><strong>MiniJinja: a powerful template engine for Rust with minimal dependencies</strong></p>

[![Build Status](https://github.com/mitsuhiko/minijinja/workflows/Tests/badge.svg?branch=main)](https://github.com/mitsuhiko/minijinja/actions?query=workflow%3ATests)
[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/d/minijinja.svg)](https://crates.io/crates/minijinja)
[![rustc 1.43.0](https://img.shields.io/badge/rust-1.43%2B-orange.svg)](https://img.shields.io/badge/rust-1.43%2B-orange.svg)
[![Documentation](https://docs.rs/minijinja/badge.svg)](https://docs.rs/minijinja)

</div>

MiniJinja is a powerful but minimal dependency template engine for Rust which
is based on the syntax and behavior of the
[Jinja2](https://jinja.palletsprojects.com/) template engine for Python.

It's implemented on top of `serde` and only has it as a single dependency. It
supports a range of features from Jinja2 including inheritance, filters and
more.  The goal is that it should be possible to use some templates in Rust
programs without the fear of pulling in complex dependencies for a small
problem.  Additionally it tries not to re-invent something but stay in line
with prior art to leverage an already existing ecosystem of editor integrations.

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

MiniJinja supports Rust versions down to 1.42 at the moment.  For the order
preservation feature Rust 1.49 is required as it uses the indexmap dependency
which no longer supports older Rust versions.

Note that we currently cannot run tests against 1.42.0 due to our tests
depending on insta which in turn also depends on indexmap.

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Documentation](https://docs.rs/minijinja/)
- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
