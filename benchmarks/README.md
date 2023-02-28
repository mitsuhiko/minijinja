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
cmp_compile/handlebars  time:   [46.856 µs 46.895 µs 46.943 µs]
cmp_compile/liquid      time:   [76.993 µs 77.069 µs 77.166 µs]
cmp_compile/minijinja   time:   [4.2695 µs 4.2790 µs 4.2881 µs]
cmp_compile/tera        time:   [85.535 µs 85.602 µs 85.685 µs]

cmp_render/askama       time:   [1.6896 µs 1.6933 µs 1.6971 µs]
cmp_render/handlebars   time:   [6.2611 µs 6.2645 µs 6.2683 µs]
cmp_render/liquid       time:   [12.013 µs 12.030 µs 12.048 µs]
cmp_render/minijinja    time:   [4.9891 µs 4.9972 µs 5.0069 µs]
cmp_render/tera         time:   [8.1737 µs 8.1826 µs 8.1925 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
