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
cmp_compile/handlebars  time:   [47.559 µs 47.671 µs 47.798 µs]
cmp_compile/liquid      time:   [35.900 µs 35.916 µs 35.932 µs]
cmp_compile/minijinja   time:   [5.9289 µs 5.9435 µs 5.9617 µs]
cmp_compile/tera        time:   [39.341 µs 39.370 µs 39.402 µs]

cmp_render/askama       time:   [1.7161 µs 1.7188 µs 1.7222 µs]
cmp_render/handlebars   time:   [6.4346 µs 6.4413 µs 6.4484 µs]
cmp_render/liquid       time:   [11.802 µs 11.810 µs 11.821 µs]
cmp_render/minijinja    time:   [5.5019 µs 5.5078 µs 5.5147 µs]
cmp_render/tera         time:   [8.0550 µs 8.0638 µs 8.0725 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
