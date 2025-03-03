<div align="center">
  <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
  <p><strong>MiniJinja for JavaScript: a powerful template engine</strong></p>

[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)

</div>

`minijinja-js` is an experimental binding of
[MiniJinja](https://github.com/mitsuhiko/minijinja) to JavaScript.  It has somewhat
limited functionality compared to the Rust version.  These bindings use
`wasm-bindgen`.

You might want to use MiniJinja instead of Jinja2 when the full feature set
of Jinja2 is not required and you want to have the same rendering experience
of a data set between Rust, Python and JavaScript.

This exposes a bunch of MiniJinja via wasm to the browser, but not all of it.

This package can be useful if you have MiniJinja templates that you want to
evaluate as a sandbox in a browser for a user or on the backend.  Given the
overheads that this creates size and performance wise, it would not be wise to
use this for actual template rendering in the browser.

## Example

Render a template from a string:

```typescript
import { Environment } from "minijinja-js";

const env = new Environment();
env.debug = true;
const result = env.renderStr('Hello {{ name }}!', { name: 'World' });
console.log(result);
```

Render a template registered to the engine:

```typescript
import { Environment } from "minijinja-js";

const env = new Environment();
env.addTemplate('index.html', 'Hello {{ name }}!');
const result = env.renderTemplate('index.html', { name: 'World' });
console.log(result);
```

Evaluate an expression:

```typescript
import { Environment } from "minijinja-js";

const env = new Environment();
const result = env.evalExpr('1 + 1', {});
console.log(result);
```

## Web Usage

If you want to use minijinja-js from the browser instead of node, you will
need to use slightly different imports and call init explicitly:


```javascript
import init, { Environment } from "minijinja-js/dist/web";
await init();
```

## Known Limitations

There are various limitations with the binding today, some of which can be fixed,
others probably not so much.  You might run into the following:

* Access of the template engine state from JavaScript is not possible.
* You cannot register a custom auto escape callback or a finalizer
* If the engine panics, the WASM runtime corrupts.

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- [MiniJinja Playground](https://mitsuhiko.github.io/minijinja-playground/)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
