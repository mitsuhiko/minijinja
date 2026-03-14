# Autoresearch ideas backlog (updated 2026-03-14, post-#121)

## Current status
- **Target:** keep 16-byte `Value` default while improving render throughput.
- **Best observed render:** **~20.65µs** (`render_ns=20648.12`) on commit `76d44fb`.
- **Recent major keeps:**
  - `9f876f7`: `From<String> for Value` now reuses owned allocation for non-small strings (no extra copy via `&str`).
  - `76d44fb`: precomputed strict/semi-strict undefined mode in VM eval and simplified Emit guard branch.
- **Practical noise band recently:** many controls land around ~20.7–21.0µs, so tiny deltas need repeated validation.

## High-confidence next directions
- Add a low-overhead multi-sample validation step for candidate keeps (avoid promoting one-run noise).
- Profile/filter-call hotspots before expanding dedicated fast-dispatch beyond `upper`.
- Continue exploring *safe* indirection reductions for object/string payloads that preserve 16-byte `Value` and semantics.

## Deferred (promising but complex)
- Replace `Object(Arc<DynObject>)` with a thinner/single-allocation object handle (likely requires unsafe/custom refcount internals).
- Replace `Arc<String>` with a truly single-allocation shared string handle that still keeps `Value` at 16 bytes.

## Pruned / already tried (stale)
- Thread-local `Arc<str>` interning for `context!` keys (regressed).
- VM cached callable classification (`BoxedFunction` fast dispatch) (regressed).
- Manual one-pass `upper` ASCII/lowercase scan variant (regressed).
- Small integer string cache in `ArgType<Cow<str>>` via `OnceLock` (regressed).
- `Context::current_loop` manual loop rewrite (regressed).
- `Context::load` precomputed `key == "loop"` branch tweak (regressed).
- Extra VM branchy `upper` one-arg micro-specialization (`call_upper_filter_one`) (regressed).
- `upper` numeric cache size tuning outside 256 (128 worse, 512 no gain).
- Replaced cached numeric HTML formatting with per-call stack decimal conversion (regressed).
- Broad SmallStr no-escape shortcut (regressed versus integer-focused shortcut).
- VM unchecked single-arg upper dispatch with manual strict guard (no gain).
- SmallStr-backed cache for numeric HTML formatting (no gain versus Box<str> cache).
- `context!` fast-path macro specializations (literal / nested `context!` / `vec!` forms) (all regressed).
- Per-eval/per-local caching of upper fast-dispatch classification in VM (regressed).
- StrMap lookup rewrites (manual loops and threshold retunes) (regressed).
- Retuning VM stack / locals preallocation sizes away from current keeps (no better-than-best result).
- Stack single-arg `get_call_args` shortcut (regressed).
- Empty safe-string singleton cache in `from_safe_string` (regressed).
- `Arc<Box<str>>` prototype for `ValueRepr::String` (regressed).
- Fixed-size array variant of upper small-int cache (`Box<[SmallStr; 256]>`) (regressed).
- VM-level `loop.index` downcast specialization (severe regression).
- Replacing `UndefinedBehavior::is_true` calls in jump/not opcodes with manual strict checks (severe regression).
