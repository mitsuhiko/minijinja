# minijinja-cli

[![Build Status](https://github.com/mitsuhiko/minijinja/workflows/Tests/badge.svg?branch=main)](https://github.com/mitsuhiko/minijinja/actions?query=workflow%3ATests)
[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/d/minijinja-contrib.svg)](https://crates.io/crates/minijinja-contrib)
[![rustc 1.61.0](https://img.shields.io/badge/rust-1.61%2B-orange.svg)](https://img.shields.io/badge/rust-1.61%2B-orange.svg)
[![Documentation](https://docs.rs/minijinja-contrib/badge.svg)](https://docs.rs/minijinja-contrib)

`minijinja-cli`` is a command line executable that uses 
[MiniJinja](https://github.com/mitsuhiko/minijinja) to render Jinja2 templates
directly from the command line to stdout.

```
minijinja-cli my-template.j2 data.json
```

## Arguments

`minijinja-cli` has two positional arguments to refer to files.  Either one of them can
be set to `-` to read from stdin.  This is the default for the template, but only one
can be set to stdin at once.

- `[TEMPLATE]`:
    the first argument is the filename of the template.  If not provided it defaults
    to `-` which means it loads the template from stdin.
- `[DATA]`:
    the second argument is the path to the data file.  This is a file which holds
    input variables that should be rendered.  Various file formats are supported.
    When data is read from `stdin`, `--format` must be specified as auto detection
    is based on file extensions.

## Options

- `-f`, `--format` `<FORMAT>`:
    this defines the input format of the data file.  The default is `auto` which
    turns on auto detection based on the file extension.  For the supported formats
    see the next section.
- `-a`, `--autoescape` `<MODE>`:
    picks an auto escape mode.  The default is auto detection (`auto`) based on
    file extension.  The options are `none` to disable escaping, `html` to
    enable HTML/XML escaping, `json` to enable JSON (YAML compatible)
    serialization.
- `-D`, `--define` `<EXPR>`:
    defines a variable from an expression.  The supported formats are `NAME` to define
    the variable `NAME` with the value `true`, `NAME=VALUE` to define the variable
    `NAME` with the value `VALUE` as string or `NAME:=VALUE` to set the variable `NAME`
    to the YAML interpreted value `VALUE`.  When YAML support is not enabled, `:=`
    only supports JSON.
- `--strict`:
    enables strict mode.  Undefined variables will then error upon rendering.
- `--no-include`:
    disallows including or extending of templates from the file system.
- `--env`:
    passes the environment variables to the template in the variable `ENV`
- `-E`, `--expr` `<EXPR>`:
    rather than rendering a template, evaluates an expression instead.  What happens
    with the result is determined by `--expr-out`.
- `--expr-out` `<MODE>`:
    sets the expression output mode.  The default is `print`.  `print` just prints
    the expression output, `json` emits it as JSON serialized value and
    `status` hides the output but reports it as exit status.  `true` converts to `0`
    and `false` converts to `1`.  Numeric results are returned unchanged.
- `--fuel` `<AMOUNT>`:
    sets the maximum fuel for the engine.  When the engine runs out of fuel it will error.
- `--repl`:
    spawns an interactive read-eval print loop for MiniJinja expressions.
- `--dump` `<KIND>`:
    prints internals of the template.  Possible options are `tokens` to see the output
    of the tokenizer, `ast` to see the AST after parsing, and `instructions` to inspect
    the compiled bytecode.
- `--version`:
    prints the version.
- `--help`:
    prints the help.

## Formats

The following formats are supported:

- `json` (`*.json`, `*.json5`): JSON5 (or JSON if JSON5 is not compiled in)
- `yaml` (`*.yaml`, `*.yml`): YAML
- `toml` (`*.toml`): TOML
- `cbor` (`*.cbor`): CBOR
- `querystring` (`*.qs`): URL encoded query strings

## Examples

Render a template with a string and integer variable:

```
minijinja-cli template.j2 -D name=World -D count:=3
```

Render a template with variables from stdin:

```
echo '{"name": "World"}' | minijinja-cli -f json template.j2 -
```

Evaluate an expression and print the result:

```
minijinja-cli --env -E "ENV.HOME or ENV.USERPROFILE"
```

REPL:

```
minijinja-cli --repl -D name=World
MiniJinja Expression REPL
Type .help for help. Use .quit or ^D to exit.
>>> name|upper
"WORLD"
>>> range(3)
[0, 1, 2]
```

## Behavior

Templates can extend other templates or include them.  Paths are relative to the
parent template.  So when you are in `foo/bar.j2` and you include `utils.j2`
it will load `foo/utils.j2`.  Including of templates can be disabled for
security reasons with `--no-include`.

All filters and functions from MiniJinja and [`minijinja-contrib`](https://docs.rs/minijinja-contrib/)
are available.

Upon failure a stack trace is rendered to stderr.

The `repl` mode lets you execute MiniJinja expressions.

## Compile-Time Features

By default all features are enabled.  The following features can be explicitly
selected when the defaults are turned off:

* `yaml`: enables YAML support
* `toml`: enables TOML support
* `cbor`: enables CBOR support
* `json5`: enables JSON5 support (instead of JSON)
* `querystring`: enables querystring support
* `datetime`: enables the date and time filters and `now()` function

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## License and Links

- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)