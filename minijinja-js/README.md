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

## Safe Strings

Strings marked as safe are not auto escaped.  Safe strings are represented
by `SafeString` (which extends `String`) and can be created with `markSafe`.
Callbacks receive safe strings (for instance after the `|safe` filter) as
`SafeString` objects and can return them to emit markup:

```typescript
import { Environment, markSafe } from "minijinja-js";

const env = new Environment();
env.addFilter("bold", (value) => markSafe(`<b>${value}</b>`));
env.addTemplate("index.html", "{{ name|bold }} {{ icon }}");
console.log(env.renderTemplate("index.html", { name: "World", icon: markSafe("<i>*</i>") }));
// -> <b>World</b> <i>*</i>
```

## Errors

Errors are raised as `TemplateError` which extends `Error` and provides
`kind`, `detail`, `templateName`, `line`, `range` and `templateSource`
properties.  If the error was caused by an exception thrown in a callback,
the exception is available as `cause`.

## Runtimes

The package is an ES module and works in Node.js, Deno, Bun, browsers and
bundlers:

* In Node.js the wasm module is loaded synchronously.  Recent Node.js versions
  can also load the package with `require()`.
* In all other environments the wasm module is loaded relative to the package
  (via `import.meta.url`) with top-level await.

If you need to control how the wasm module is loaded (for instance on
Cloudflare Workers or with a custom asset pipeline), use the `minijinja-js/init`
entry point and initialize the module yourself before use:

```javascript
import { init, initSync, Environment } from "minijinja-js/init";

// load from a URL, Response or compiled module
await init(new URL("minijinja_js_bg.wasm", someBaseUrl));

// or synchronously from bytes or a compiled module
initSync(wasmModule);
```

The wasm file is exported as `minijinja-js/minijinja_js_bg.wasm`.

## Known Limitations

There are various limitations with the binding today, some of which can be fixed,
others probably not so much.  You might run into the following:

* Access of the template engine state from JavaScript is not possible.
* Filters, tests and functions cannot be async.
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
