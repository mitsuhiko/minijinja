# Autoresearch: optimize MiniJinja render benchmark

## Objective
Improve rendering speed across multiple diverse workloads used by MiniJinja's benchmark suite.
The optimization target is runtime execution (render path), not compile/parse speed.

## Metrics
- **Primary**: `render_ns` — sum of all three workload render times (ns, lower is better)
- **Secondary**: `render_all_elements_ns`, `render_string_heavy_ns`, `render_macro_heavy_ns`, `parse_ns`, `compile_ns`

## Workloads
Three complementary templates to prevent overfitting to any single pattern:
1. **all_elements.html** — loop-heavy (197 integer items with `|upper`), blocks, includes, conditionals
2. **string_heavy.html** — string filters (`title`, `upper`, `lower`, `replace`, `join`, `default`), HTML escaping, nested object attribute access, varied string lengths
3. **macro_heavy.html** — macro definitions and calls, conditionals, metadata iteration (`|items`), mixed data types, nested context

## How to Run
`./autoresearch.sh` — emits `METRIC name=number` lines.

## Files in Scope
- `minijinja/src/template.rs` — template render entry points and output allocation.
- `minijinja/src/output.rs` — output buffering and capture behavior.
- `minijinja/src/vm/mod.rs` — VM execution hot path.
- `minijinja/src/value/` — value operations used during rendering.
- `benchmarks/src/bin/autoresearch_render.rs` — dedicated fast benchmark harness.
- `benchmarks/inputs/` — benchmark template files.
- `autoresearch.sh` — benchmark runner.

## Off Limits
- Public API behavior/semantics.
- Feature-gating behavior.
- Crates outside MiniJinja workspace not related to benchmark harness.

## Constraints
- Optimizations must benefit at least two of three workloads (no single-workload micro-optimizations).
- Keep benchmark workload semantics equivalent to existing benchmarks.
- No new external dependencies.
- Build must pass for touched code (`cargo check -p benchmarks --bin autoresearch_render`).
- Do not change iteration order of internal data structures (e.g., Locals must preserve BTreeMap sort order).

## What's Been Tried
- Added an ASCII no-op fast path to `upper` filter.
- Added `needs_html_escaping` pre-check and direct `out.write_str` fast path for unescaped strings.
- Added `formatter_is_default` fast path and VM-side `Emit` specialization for default formatter.
- Added `Object::get_value_by_str` to avoid temporary `Value` key construction for string attribute access.
- Switched `context!` internal map from `BTreeMap<Value, Value>` to `BTreeMap<Arc<str>, Value>`.
- Specialized small `Value`-keyed map string lookup by matching directly on `ValueRepr::String`/`SmallStr`.
- Tuned small-map linear-scan threshold from `<=8` to `<=12`.
- Added `Loop::get_value_by_str` with "index" prioritized.
- Tuned VM value stack initial capacity from 16 to 24.
- Tuned context frame-stack initial capacity from 32 to 40.
- Added primitive fast paths in `ArgType<Cow<str>>` for u64/i64/bool.
- Added small-integer HTML output fast paths for U64/I64/Bool.
- Added SmallStr integer-string fast path in HTML escaping writer.
- Precomputed strict/semi-strict undefined mode once per VM eval.
- **Reverted**: Vec-backed Locals — broke iteration order guarantees used by module export with `preserve_order`.
- **Reverted**: 16-byte Value — too complex with insufficient generic benefit.
- **Reverted**: Upper-filter-specific optimizations (dedicated opcode, UpperFilterObject, integer cache) — single-filter micro-optimizations.
