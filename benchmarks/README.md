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
cmp_compile/handlebars  time:   [46.691 µs 46.749 µs 46.810 µs]
cmp_compile/liquid      time:   [35.316 µs 35.362 µs 35.417 µs]
cmp_compile/minijinja   time:   [5.8038 µs 5.8112 µs 5.8186 µs]
cmp_compile/tera        time:   [37.483 µs 37.548 µs 37.620 µs]

cmp_render/handlebars   time:   [6.4022 µs 6.4082 µs 6.4149 µs]
cmp_render/liquid       time:   [11.505 µs 11.525 µs 11.548 µs]
cmp_render/minijinja    time:   [5.3910 µs 5.4023 µs 5.4148 µs]
cmp_render/tera         time:   [7.8721 µs 7.8802 µs 7.8896 µs]
```
