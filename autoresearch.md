# Autoresearch: optimize MiniJinja render benchmark

## Objective
Improve rendering speed of the `all_elements.html` workload used by MiniJinja's benchmark suite.
The optimization target is runtime execution (render path), not compile/parse speed.

## Metrics
- **Primary**: `render_ns` (ns, lower is better)
- **Secondary**: `render_mean_ns`

## How to Run
`./autoresearch.sh` — emits `METRIC name=number` lines.

## Files in Scope
- `minijinja/src/template.rs` — template render entry points and output allocation.
- `minijinja/src/output.rs` — output buffering and capture behavior.
- `minijinja/src/vm/mod.rs` — VM execution hot path.
- `minijinja/src/value/` — value operations used during rendering.
- `benchmarks/src/bin/autoresearch_render.rs` — dedicated fast benchmark harness.
- `autoresearch.sh` — benchmark runner.

## Off Limits
- Public API behavior/semantics.
- Feature-gating behavior.
- Crates outside MiniJinja workspace not related to benchmark harness.

## Constraints
- Keep benchmark workload semantics equivalent to existing benchmark (`benches/templates.rs` render case).
- No new external dependencies.
- Build must pass for touched code (`cargo check -p benchmarks --bin autoresearch_render`).

## What's Been Tried
- Initial setup with dedicated benchmark harness for fast render-loop iteration.
