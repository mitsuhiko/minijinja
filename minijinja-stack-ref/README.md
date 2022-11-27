# MiniJinja-Stack-Ref

[![Build Status](https://github.com/mitsuhiko/minijinja/workflows/Tests/badge.svg?branch=main)](https://github.com/mitsuhiko/minijinja/actions?query=workflow%3ATests)
[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/d/minijinja-stack-ref.svg)](https://crates.io/crates/minijinja-stack-ref)
[![rustc 1.61.0](https://img.shields.io/badge/rust-1.61%2B-orange.svg)](https://img.shields.io/badge/rust-1.61%2B-orange.svg)
[![Documentation](https://docs.rs/minijinja-stack-ref/badge.svg)](https://docs.rs/minijinja-stack-ref)

MiniJinja-Stack-Ref is a utility crate for [MiniJinja](https://github.com/mitsuhiko/minijinja)
that adds support for borrowing of dynamic values from the stack.

```rust
use minijinja::{context, Environment};
use minijinja_stack_ref::scope;

let mut env = Environment::new();
env.add_template(
    "info",
    "app version: {{ state.version }}\nitems: {{ items }}"
)
.unwrap();

let items = [1u32, 2, 3, 4];
let rv = scope(|scope| {
    let tmpl = env.get_template("info").unwrap();
    tmpl.render(context! {
        items => scope.seq_object_ref(&items[..]),
    }).unwrap()
});
println!("{}", rv);
```

For an example have a look at the [stack-ref example](https://github.com/mitsuhiko/minijinja/tree/main/examples/stack-ref).

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Documentation](https://docs.rs/minijinja-stack-ref/)
- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
