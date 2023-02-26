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
cmp_compile/handlebars  time:   [47.679 µs 47.807 µs 47.994 µs]
cmp_compile/liquid      time:   [77.487 µs 77.618 µs 77.778 µs]
cmp_compile/minijinja   time:   [4.3142 µs 4.3264 µs 4.3398 µs]
cmp_compile/tera        time:   [86.599 µs 86.754 µs 86.942 µs]

cmp_render/askama       time:   [1.7090 µs 1.7109 µs 1.7130 µs]
cmp_render/handlebars   time:   [6.2485 µs 6.2621 µs 6.2787 µs]
cmp_render/liquid       time:   [12.089 µs 12.120 µs 12.163 µs]
cmp_render/minijinja    time:   [5.0464 µs 5.0580 µs 5.0718 µs]
cmp_render/tera         time:   [8.1701 µs 8.1905 µs 8.2214 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
