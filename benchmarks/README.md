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
cmp_compile/handlebars  time:   [47.102 µs 47.234 µs 47.406 µs]
cmp_compile/liquid      time:   [77.832 µs 78.082 µs 78.387 µs]
cmp_compile/minijinja   time:   [4.7069 µs 4.7191 µs 4.7335 µs]
cmp_compile/tera        time:   [85.692 µs 85.948 µs 86.244 µs]

cmp_render/askama       time:   [1.6835 µs 1.6854 µs 1.6876 µs]
cmp_render/handlebars   time:   [6.2346 µs 6.2484 µs 6.2663 µs]
cmp_render/liquid       time:   [12.024 µs 12.041 µs 12.062 µs]
cmp_render/minijinja    time:   [4.9884 µs 5.0016 µs 5.0166 µs]
cmp_render/tera         time:   [8.0609 µs 8.0681 µs 8.0761 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
