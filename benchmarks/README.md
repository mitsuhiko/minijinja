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
cmp_compile/handlebars  time:   [47.700 µs 47.738 µs 47.779 µs]
cmp_compile/liquid      time:   [37.231 µs 37.263 µs 37.297 µs]
cmp_compile/minijinja   time:   [6.2611 µs 6.2711 µs 6.2814 µs]
cmp_compile/tera        time:   [37.018 µs 37.049 µs 37.080 µs]

cmp_render/handlebars   time:   [7.8582 µs 7.8676 µs 7.8791 µs]
cmp_render/liquid       time:   [12.444 µs 12.497 µs 12.591 µs]
cmp_render/minijinja    time:   [5.9009 µs 5.9055 µs 5.9105 µs]
cmp_render/tera         time:   [9.4471 µs 9.4584 µs 9.4711 µs]
```
