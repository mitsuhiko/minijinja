# Python-Compatible Rendering Example

This example demonstrates MiniJinja's Python-compatible rendering mode.

## Background

By default, MiniJinja renders values in its own style:
- Booleans: `true`/`false`
- None: `none`
- Strings: Rust-style escaping and quoting

When Python compatibility is needed (e.g., for SQL templating where precise output matters), you can enable PyCompat mode to match Python Jinja2's output:
- Booleans: `True`/`False`
- None: `None`
- Strings: Python-style escaping and quoting

## Usage

```rust
use minijinja::Environment;

let mut env = Environment::new();

// Enable Python-compatible rendering
env.set_pycompat_rendering(true);

// Now templates will render values like Python Jinja2
let tmpl = env.template_from_str("{{ [true, false, none, 'hello'] }}")?;
let result = tmpl.render(minijinja::context!{})?;
// Output: [True, False, None, 'hello']
```

## Running this Example

```bash
cargo run --example pycompat-rendering
```