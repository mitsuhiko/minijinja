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
cmp_compile/handlebars  time:   [49.106 µs 49.259 µs 49.418 µs]
cmp_compile/liquid      time:   [38.069 µs 38.168 µs 38.271 µs]
cmp_compile/minijinja   time:   [4.3756 µs 4.3878 µs 4.4014 µs]
cmp_compile/tera        time:   [42.156 µs 42.291 µs 42.422 µs]

cmp_render/askama       time:   [1.2700 µs 1.2732 µs 1.2768 µs]
cmp_render/handlebars   time:   [5.8255 µs 5.8433 µs 5.8610 µs]
cmp_render/liquid       time:   [11.292 µs 11.334 µs 11.376 µs]
cmp_render/minijinja    time:   [4.4880 µs 4.4976 µs 4.5069 µs]
cmp_render/rinja        time:   [916.30 ns 920.48 ns 924.88 ns]
cmp_render/tera         time:   [6.9698 µs 6.9978 µs 7.0277 µs]
```

Note that Askama compiles templates as part of the Rust build
process and uses static typing, as such it does not have a compile
time benchmark.
