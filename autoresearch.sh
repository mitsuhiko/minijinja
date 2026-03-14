#!/usr/bin/env bash
set -euo pipefail

# Fast sanity check for syntax/type errors before benchmarking.
cargo check -q -p benchmarks --bin autoresearch_comparison --bin autoresearch_render

# Primary target: comparison benchmark workload used in benches/comparison.rs.
cargo run --quiet --release -p benchmarks --bin autoresearch_comparison

# Secondary guardrails: existing multi-workload benchmark suite.
cargo run --quiet --release -p benchmarks --bin autoresearch_render
