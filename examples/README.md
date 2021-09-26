# Examples

The examples in this folder show a bit what one can do with the MiniJinja template engine.

## `expr`

This demonstrates how MiniJinja can be used to evaluate expressions.  It accepts a single
argument which is an expression that should be evaluated and the result is printed in JSON
format to stdout.  A single variable is provided to the script (`env`) which contains the
environment variables.

```console
$ cargo run  --example expr -- 'env.HOME ~ "/.bashrc"'
"/Users/mitsuhiko/.bashrc"
```

## `hello`

The most straightforward hello world template rendered:

```console
$ cargo run  --example hello
```

## `inheritance`

Demonstrates template inheritance.

```console
$ cargo run  --example inheritance
```