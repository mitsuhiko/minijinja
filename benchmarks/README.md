# Benchmarks

This is the beginning of a basic benchmarking suite.  So far it doesn't do much.
These use [criterion.rs](https://github.com/bheisler/criterion.rs) for testing.

There are some benchmarks for the engine itself to track changes over time and
comparison benchmarks against `handlebars`, `liquid` and `tera`.

To run the benchmarks:

```
$ cargo bench
```

## Comparison Results

These are the results run on a MacBook Pro 16" 2021:

```
cmp_compile/handlebars  time:   [47.935 µs 47.981 µs 48.031 µs]
cmp_compile/liquid      time:   [37.415 µs 37.483 µs 37.566 µs]
cmp_compile/minijinja   time:   [6.4807 µs 6.4920 µs 6.5035 µs]
cmp_compile/tera        time:   [37.738 µs 37.791 µs 37.843 µs]

cmp_render/handlebars   time:   [7.9399 µs 7.9572 µs 7.9742 µs]
cmp_render/liquid       time:   [12.566 µs 12.590 µs 12.613 µs]
cmp_render/minijinja    time:   [5.7659 µs 5.7765 µs 5.7871 µs]
cmp_render/tera         time:   [9.4953 µs 9.5125 µs 9.5302 µs]
```
