#!/usr/bin/env bash
set -euo pipefail

# Fast sanity check for syntax/type errors before benchmarking.
cargo check -q -p benchmarks --bin autoresearch_render

# Multi-workload benchmark: all_elements (loops/integers), string_heavy (filters/escaping),
# macro_heavy (macros/conditionals/metadata). Primary metric is the sum of all three.
cargo run --quiet --release -p benchmarks --bin autoresearch_render
