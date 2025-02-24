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
cmp_compile/handlebars  time:   [48.943 µs 49.046 µs 49.159 µs]
cmp_compile/liquid      time:   [37.338 µs 37.526 µs 37.823 µs]
cmp_compile/minijinja   time:   [4.3509 µs 4.3762 µs 4.4141 µs]
cmp_compile/tera        time:   [43.423 µs 43.691 µs 43.975 µs]

cmp_render/askama       time:   [1.3078 µs 1.3125 µs 1.3170 µs]
cmp_render/handlebars   time:   [6.1760 µs 6.2002 µs 6.2253 µs]
cmp_render/liquid       time:   [11.241 µs 11.283 µs 11.326 µs]
cmp_render/minijinja    time:   [4.5419 µs 4.5661 µs 4.5929 µs]
cmp_render/rinja        time:   [922.53 ns 925.72 ns 929.00 ns]
cmp_render/tera         time:   [6.9522 µs 6.9731 µs 6.9954 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
