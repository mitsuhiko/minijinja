# Autoresearch ideas backlog (updated 2026-03-14, latest loop)

## Current status
- **Target:** keep 16-byte `Value` default while improving render throughput.
- **Best valid render so far:** ~**23.35µs** (`render_ns=23347.66`) on commit `5a0fab7`.
- Biggest confirmed wins now are:
  - `Seq` + `StrMap` specialized `ValueRepr` variants,
  - VM locals lookup simplification,
  - specialized built-in `upper` path:
    - dedicated `UpperFilterObject` registration (bypasses generic `from_function` arg marshalling),
    - VM-side fast dispatch for builtin `upper`,
    - cached small-integer `Value` coercions (0..255) for numeric `upper` calls.

## High-confidence next directions
- Profile whether similar dedicated-object dispatch is worthwhile for other *actually hot* builtins before broad rollout.
- Explore reducing object indirection cost beyond `StrMap`/`Seq` (thin object handle design) while preserving semantics.
- Investigate thin shared string representation to avoid `Arc<String>` double-allocation tradeoff without giving up 16-byte `Value`.

## Deferred (promising but complex)
- Replace `Object(Arc<DynObject>)` with a thinner single-allocation object handle (likely unsafe/custom refcount internals).
- Replace `Arc<String>` payload with single-allocation thin shared string object.

## Pruned / already tried (stale)
- Thread-local `Arc<str>` interning for `context!` keys (regressed).
- VM cached callable classification (`BoxedFunction` fast dispatch) (regressed).
- Manual one-pass `upper` ASCII/lowercase scan variant (regressed).
- Small integer string cache in `ArgType<Cow<str>>` via `OnceLock` (regressed).
- `Context::current_loop` manual loop rewrite (regressed).
- `Context::load` precomputed `key == "loop"` branch tweak (regressed).
- Extra VM branchy `upper` one-arg micro-specialization (`call_upper_filter_one`) (regressed).
- `upper` numeric cache size tuning outside 256 (128 worse, 512 no gain).
