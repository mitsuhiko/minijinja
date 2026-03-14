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

These are the latest results run on a MacBook Pro 16" (2021) with:

```
$ cargo bench -p benchmarks --bench comparison
```

```
cmp_compile/handlebars  time:   [65.693 µs 65.828 µs 65.962 µs]
cmp_compile/liquid      time:   [67.570 µs 67.704 µs 67.841 µs]
cmp_compile/minijinja   time:   [3.8695 µs 3.8772 µs 3.8847 µs]
cmp_compile/tera        time:   [63.253 µs 63.610 µs 64.144 µs]

cmp_render/askama       time:   [1.4681 µs 1.5798 µs 1.7433 µs]
cmp_render/handlebars   time:   [8.8205 µs 8.8373 µs 8.8551 µs]
cmp_render/liquid       time:   [12.878 µs 12.900 µs 12.921 µs]
cmp_render/minijinja    time:   [3.7371 µs 3.7446 µs 3.7530 µs]
cmp_render/rinja        time:   [935.29 ns 937.13 ns 938.86 ns]
cmp_render/tera         time:   [6.8399 µs 6.8598 µs 6.8825 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
