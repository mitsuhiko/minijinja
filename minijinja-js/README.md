<div align="center">
  <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
  <p><strong>MiniJinja for JavaScript: a powerful template engine for Rust and Python</strong></p>

[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/d/minijinja.svg)](https://crates.io/crates/minijinja)
[![rustc 1.63.0](https://img.shields.io/badge/rust-1.63%2B-orange.svg)](https://img.shields.io/badge/rust-1.63%2B-orange.svg)
[![Documentation](https://docs.rs/minijinja/badge.svg)](https://docs.rs/minijinja)

</div>

`minijinja-js` is an experimental binding of
[MiniJinja](https://github.com/mitsuhiko/minijinja) to JavaScript.  It has somewhat
limited functionality compared to the Rust version.  These bindings use
`wasm-bindgen`.

You might want to use MiniJinja instead of Jinja2 when the full feature set
of Jinja2 is not required and you want to have the same rendering experience
of a data set between Rust, Python and JavaScript.

This exposes a bunch of MiniJinja via wasm to the browser.  Due to limitations you
can only render templates and register globals, but you cannot register callbacks.
This might be something that can be changed in the future.  However as a result it's
currently not possible to customize this package for more elaborate uses.

## Example

```typescript
import { Environment } from "minijinja-js";

const env = new Environment();
env.debug = true;
const result = env.renderStr('Hello {{ name }}!', { name: 'World' });
console.log(result);
```

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Documentation](https://docs.rs/minijinja/)
- [Examples](https://github.com/mitsuhiko/minijinja/tree/main/examples)
- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- [MiniJinja Playground](https://mitsuhiko.github.io/minijinja-playground/)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
