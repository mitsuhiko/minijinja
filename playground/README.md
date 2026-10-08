# MiniJinja Playground

A browser playground for MiniJinja built on top of
[minijinja-js](../minijinja-js).  It is deployed to
<https://mitsuhiko.github.io/minijinja/> from the `main` branch by the
`playground` GitHub Actions workflow.

Features:

* Multiple templates (for `extends`, `include` and `import`) with a
  selectable entry template
* JSON context with completions of context variables in templates
* JavaScript config code to register filters, tests, functions and globals
  (config code from shared links only runs after confirmation)
* Rendered output (with optional whitespace visualization), a preview (HTML
  or text) and the undeclared variables of a template
* Tokens, AST and VM instructions of the current template
* Configurable syntax, whitespace handling and undefined behavior
* Date and time filters (from `minijinja-js/datetime`) and random functions
* The state is kept in the URL so links can be shared
* Templates render in a web worker and are aborted if they do not finish

## Development

Requires Rust with the `wasm32-unknown-unknown` target,
[wasm-pack](https://drager.github.io/wasm-pack/) and Node.js.

```bash
npm run build:wasm  # builds minijinja-js with the unstable machinery
npm install
npm run dev
```

The playground uses the `unstable_machinery` feature of minijinja-js to show
tokens, the AST and instructions.  This feature is not enabled for the
published npm package.
