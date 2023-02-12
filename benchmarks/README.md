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
cmp_compile/handlebars  time:   [49.507 µs 49.559 µs 49.613 µs]
cmp_compile/liquid      time:   [81.102 µs 81.198 µs 81.301 µs]
cmp_compile/minijinja   time:   [4.9363 µs 4.9433 µs 4.9506 µs]
cmp_compile/tera        time:   [89.963 µs 90.059 µs 90.156 µs]

cmp_render/askama       time:   [1.8098 µs 1.8121 µs 1.8145 µs]
cmp_render/handlebars   time:   [6.5743 µs 6.5821 µs 6.5911 µs]
cmp_render/liquid       time:   [12.604 µs 12.623 µs 12.642 µs]
cmp_render/minijinja    time:   [5.4388 µs 5.4460 µs 5.4534 µs]
cmp_render/tera         time:   [8.4889 µs 8.5013 µs 8.5157 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
