# MiniJinja development guide

## Before working

- Check for `.ai-ack` next to `.git`. If it is missing, read
  `HUMAN_VS_MACHINE.md` and follow it before making changes.
- Follow `CONTRIBUTING.md`, including its AI-assistance disclosure requirement
  for pull requests.

## Current development lines

- `main` is the MiniJinja 3 development line. MiniJinja 2 is maintained on
  `minijinja-2`; changes for v2 should target that branch and are merged forward
  into `main`, never backward.
- `UPDATING.md` documents the v2-to-v3 API and behavior changes.
- The Rust MSRV is 1.70. CI uses `Cargo.lock.msrv` for MSRV jobs, so avoid APIs
  newer than 1.70 in MSRV crates.
- In v3, Serde support is optional and disabled by default. Preserve
  no-default-feature and feature-gated builds.

## Repository layout

- `minijinja/`: core Rust engine.
- `minijinja-{autoreload,contrib,embed,cli}/`: supporting Rust crates and CLI.
- `minijinja-{cabi,js,py}/`: C, JavaScript/WASM, and Python bindings.
- `minijinja-go/`: native Go port; it is not part of the Cargo workspace.
- `examples/` and `benchmarks/`: workspace examples and benchmarks.

Core engine flow:

1. `minijinja/src/compiler/` lexes, parses, and compiles templates to
   instructions.
2. `minijinja/src/vm/` executes those instructions.
3. `minijinja/src/value/` implements values, objects, conversions, and optional
   Serde integration.
4. `environment.rs`, `template.rs`, and `expression.rs` expose the main API;
   built-ins live primarily in `filters.rs`, `functions.rs`, and `tests.rs`.

Core integration tests are in `minijinja/tests/`. Fixture-based template,
lexer, and parser tests use inputs and Insta snapshots under that directory.

## Commands

Run focused tests first. Core tests should normally enable all features because
several integration tests are feature-gated:

```bash
cargo test -p minijinja --all-features
cargo test -p minijinja --all-features test_name -- --nocapture
```

Repository checks:

```bash
make build          # build the Cargo workspace
make check          # important feature combinations
make format         # cargo fmt
make format-check   # CI formatting check
make lint           # clippy with warnings denied
make test           # full default suite, including C ABI and Go tests
```

Additional suites are separate; run them when touching the corresponding code:

```bash
make python-test
make js-test
make wasi-test
```

Do not use `cargo insta review` or `make snapshot-tests`; they launch an
interactive reviewer. Inspect generated `.snap.new` files directly. If the
changes are intentional, accept them non-interactively from `minijinja/` with
`cargo insta test --all-features --accept`.

Before finishing, format changed Rust code and run the narrowest relevant tests
plus `make check`/`make lint` when practical. Report any checks not run.
