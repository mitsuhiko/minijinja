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
cmp_compile/handlebars  time:   [47.109 µs 47.215 µs 47.332 µs]
cmp_compile/liquid      time:   [77.468 µs 77.640 µs 77.870 µs]
cmp_compile/minijinja   time:   [4.3760 µs 4.3878 µs 4.3990 µs]
cmp_compile/tera        time:   [85.442 µs 85.675 µs 85.967 µs]

cmp_render/askama       time:   [1.6964 µs 1.7009 µs 1.7054 µs]
cmp_render/handlebars   time:   [6.2795 µs 6.2957 µs 6.3121 µs]
cmp_render/liquid       time:   [12.191 µs 12.225 µs 12.263 µs]
cmp_render/minijinja    time:   [5.0802 µs 5.0914 µs 5.1037 µs]
cmp_render/tera         time:   [8.1739 µs 8.1943 µs 8.2152 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
