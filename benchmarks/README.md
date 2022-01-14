# Benchmarks

This is the beginning of a basic benchmarking suite.  So far it doesn't do much.
These use [criterion.rs](https://github.com/bheisler/criterion.rs) for testing.

To run the benchmarks:

```
$ cargo bench
```

If you want to run the benchmarks against MiniJinja with speedups:

```
$ cargo bench --features=benchmarks
```
