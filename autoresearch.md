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
- Tried an `Output` string-target fast path with extra branching in `write_str`/`write_char`; it regressed (~+1.8%), likely from branch overhead in the hottest path.
- Added an ASCII no-op fast path to `upper` filter (`return v.into_owned()` when no lowercase ASCII exists). Big win on this workload (many numeric `item|upper` calls), ~7% faster.
- Added `needs_html_escaping` pre-check and direct `out.write_str` fast path for unescaped strings. This avoids `write!`/`Display` overhead for common safe-looking strings and improved render throughput by another ~4%.
- Added `formatter_is_default` fast path in `Environment::format` to call `write_escaped` directly and skip dynamic formatter dispatch when default formatter is active.
- Added VM-side `Emit` specialization for default formatter to bypass `Environment::format` call overhead in the hottest output path.
- Added `Object::get_value_by_str` and routed attribute lookup through it to avoid constructing temporary `Value` keys for string attribute access.
- Switched hidden `context!` internal map representation from `ValueMap` (`BTreeMap<Value, Value>`) to `BTreeMap<Arc<str>, Value>`, reducing key conversion overhead during context construction and lookup.
- Specialized small `Value`-keyed map string lookup by matching directly on `ValueRepr::String`/`SmallStr` instead of calling generic `as_str` conversion.
- Added `Loop::get_value_by_str` override and routed `get_value` through it, removing temporary `Value` key construction for frequent `loop.<attr>` lookups.
