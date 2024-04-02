# MiniJinja Examples

This directory contains a lot of different examples that use the MiniJinja
engine.  Each example is in one directory from where you can run them with
the `cargo run` command.  Alternatively you can do `cargo run -p example-name`.

## List of Examples

* [actix-web-demo](actix-web-demo): shows how to use MiniJinja with actix web.
* [autoreload](autoreload): shows how to use auto reloading.
* [build-script](build-script): Demonstrates how to generate Rust code with MiniJinja in build scripts.
* [call-block-function](call-block-function): Shows how to use the `{% call %}` block with a custom function.
* [custom-loader](custom-loader): shows how to load templates dynamically at runtime with a custom loader.
* [debug](debug): contains an example showing the built-in `debug()` function.
* [dsl](dsl): shows how to use MiniJinja has a DSL.
* [dynamic-context](dynamic-context): demonstrates how to use dynamic objects as template context.
* [dynamic-objects](dynamic-objects): demonstrates how to use dynamic objects in templates.
* [deserialize](deserialize): demonstrates how you can deserialize directly from a value.
* [error](error): shows the built-in error reporting support.
* [eval-to-state](eval-to-state): Demonstrates what can be done with evaluating to state.
* [expr](expr): demonstrates the expression evaluation support.
* [filters](filters): Shows how to write and use custom filters and global functions.
* [generate-yaml](generate-yaml): renders YAML files from Jinja templates.
* [hello](hello): minimal Hello World example.
* [inheritance](inheritance): demonstrates how to use template inheritance.
* [embedding](embedding): shows how to use `minijina-embed` to embed templates optionally into the binary.
* [invalid-value](invalid-value): demonstrates how the engine deals with invalid values.
* [load-lazy](load-lazy): Demonstrates how to load data lazy on demand.
* [load-resource](load-resource): Demonstrates how to load files dynamically from disk within templates.
* [macros](macros): Demonstrates how to use macros and imports.
* [merge-context](merge-context): Shows how a context can be merged from more than one value.
* [minimal](minimal): a Hello World example without default features.
* [none-is-undefined](none-is-undefined): shows how MiniJinja can be configured to treat `None` like `undefined`.
* [path-loader](path-loader): Demonstrates how to load templates from disk with the `loader` feature.
* [recursive-for](recursive-for): demonstrates the recursive for loop.
* [render-macro](render-macro): minimal Hello World example using the `render!` macro.
* [render-template](render-template): CLI app that renders templates from string.
* [render-value](render-value): Demonstrates how `Value` can be passed as `Serialize` as context.
* [self-referential-context](self-referential-context): Shows a helper that allows self-referential contexts.
- [streaming](streaming): Demonstrates how a one-shot iterator can be used to stream results in.
* [undefined-tracking](undefined-tracking): Shows how you can track undefined values.
* [value-tracking](value-tracking): Shows how you can track values that are referenced at runtime.
* [wasm-yew](wasm-yew): Shows how to use MiniJinja with WASM in the browser.

## Third-party Examples

* [Actix Web Integration](https://github.com/actix/examples/blob/master/templating/minijinja)
