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

Resolve includes/extends from the filesystem (Node, etc...):

```typescript
import { Environment } from "minijinja-js";
import fs from "node:fs";
import path from "node:path";

const env = new Environment();

// Resolve relative paths like "./partial.html" against the parent template
env.setPathJoinCallback((name, parent) => {
  const parentDir = parent ? path.dirname(parent) : process.cwd();
  const joined = path.resolve(parentDir, name);
  return joined.replace(/\\\\/g, '/');
});

// Synchronous loader: return template source or null/undefined if missing
env.setLoader((name) => {
  try {
    return fs.readFileSync(name, "utf8");
  } catch {
    return null;
  }
});

// Example: main in-memory, include from disk under ./templates/dir/inc.html
const templatePath = path.resolve(process.cwd(), "templates/dir/main.html");
env.addTemplate(templatePath, "Hello {% include './inc.html' %}!");
console.log(env.renderTemplate(templatePath, { value: "World" }));
// -> Hello [World]!
```

Evaluate an expression:

```typescript
import { Environment } from "minijinja-js";

const env = new Environment();
const result = env.evalExpr('1 + 1', {});
console.log(result);
```

Register filters, tests and functions:

```typescript
import { Environment } from "minijinja-js";

const env = new Environment();
env.addFilter("repeat", (value, times, { sep = "" } = {}) =>
  Array(times).fill(value).join(sep)
);
env.addGlobal("double", (x) => x * 2);
console.log(env.renderStr("{{ 'ab'|repeat(3, sep='-') }} {{ double(21) }}"));
// -> ab-ab-ab 42
```

Keyword arguments are passed to JavaScript callbacks as a trailing object.
Exceptions thrown by callbacks fail the render with an error whose `cause`
is the original exception.

## Values

Values passed to templates are converted as follows:

* `undefined` is undefined and `null` is none.
* Numbers that are safe integers become integers, all others floats.
  `BigInt`s become integers.
* Arrays, `Set`s and typed arrays become sequences, `Uint8Array` and
  `ArrayBuffer` become bytes.
* Plain objects and `Map`s become maps.  Key order is preserved.
* `Date`s become ISO 8601 strings.
* Functions become callable.
* Other objects (for instance class instances) are not copied but accessed
  lazily.  This means getters work and methods are invoked with the object
  as `this`.

Values returned to JavaScript (from `evalExpr` or as arguments to
callbacks) are converted back:

* undefined becomes `undefined`, none becomes `null`.
* Integers outside of the safe integer range become `BigInt`s.
* Sequences become arrays, bytes become `Uint8Array`s.
* Maps with only string keys become plain objects, other maps become `Map`s.
* Functions and objects that came from JavaScript are passed back unchanged.

MiniJinja tuples retain tuple rendering inside templates. Expression results are
returned as JavaScript arrays because JavaScript has no distinct tuple type.

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
* The loader is synchronous; use sync I/O in Node etc... (e.g. `fs.readFileSync`)
* The environment cannot be modified while it renders (for instance from
  within a filter).
* If the engine panics, the WASM runtime corrupts.  This should not happen
  but if it does, please report it as a bug.

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- [MiniJinja Playground](https://mitsuhiko.github.io/minijinja-playground/)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
