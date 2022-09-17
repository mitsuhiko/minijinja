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
cmp_compile/handlebars  time:   [47.375 µs 47.430 µs 47.514 µs]
cmp_compile/liquid      time:   [36.824 µs 36.851 µs 36.883 µs]
cmp_compile/minijinja   time:   [6.2466 µs 6.2609 µs 6.2852 µs]
cmp_compile/tera        time:   [37.502 µs 37.524 µs 37.548 µs]

cmp_render/handlebars   time:   [7.9784 µs 8.0407 µs 8.1466 µs]
cmp_render/liquid       time:   [12.485 µs 12.503 µs 12.523 µs]
cmp_render/minijinja    time:   [6.5934 µs 6.6028 µs 6.6141 µs]
cmp_render/tera         time:   [9.3552 µs 9.3606 µs 9.3663 µs]
```
