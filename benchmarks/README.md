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

These are the latest results run on a MacBook Pro (Apple M5 Max) with:

```
$ cargo bench -p benchmarks --bench comparison
```

```
cmp_compile/handlebars  time:   [35.652 µs 35.781 µs 35.932 µs]
cmp_compile/liquid      time:   [40.724 µs 40.904 µs 41.098 µs]
cmp_compile/minijinja   time:   [2.4564 µs 2.4629 µs 2.4714 µs]
cmp_compile/tera        time:   [37.050 µs 37.389 µs 37.781 µs]

cmp_render/askama       time:   [817.13 ns 819.78 ns 822.49 ns]
cmp_render/handlebars   time:   [5.1375 µs 5.1459 µs 5.1541 µs]
cmp_render/liquid       time:   [6.6112 µs 6.6305 µs 6.6489 µs]
cmp_render/minijinja    time:   [2.2309 µs 2.2380 µs 2.2464 µs]
cmp_render/rinja        time:   [562.52 ns 565.36 ns 568.93 ns]
cmp_render/tera         time:   [4.0690 µs 4.0863 µs 4.1083 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
