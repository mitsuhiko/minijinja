#!/usr/bin/env bash
# Prepares the workspace for building on the minimum supported Rust version.
#
# minijinja-cli and minijinja-js have a higher MSRV than the library crates
# and their dependencies require versions of shared dependencies (for
# instance `zmij` or `js-sys`) that do not build or resolve on the library
# MSRV.  As a workspace has a single lock file, these crates are excluded
# from the workspace for MSRV builds and Cargo.lock.msrv is resolved without
# them.
#
# To update Cargo.lock.msrv, run this script and then use an MSRV aware
# resolution, e.g.:
#
#   CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo +1.70.0 update -p <crate>
#   cp Cargo.lock Cargo.lock.msrv
set -euo pipefail

cd "$(dirname "$0")/.."

if ! grep -q '"minijinja-cli"' Cargo.toml; then
  sed -i.bak 's/^exclude = \[/exclude = ["minijinja-cli", "minijinja-js", /' Cargo.toml
  rm Cargo.toml.bak
fi
grep -q '^exclude = \["minijinja-cli", "minijinja-js"' Cargo.toml

cp Cargo.lock.msrv Cargo.lock
