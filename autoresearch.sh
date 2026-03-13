#!/usr/bin/env bash
set -euo pipefail

# Fast sanity check for syntax/type errors before benchmarking.
cargo check -q -p benchmarks --bin autoresearch_render

# Primary workload benchmark.
cargo run --quiet --release -p benchmarks --bin autoresearch_render
