# Value Size Reduction: Performance Recovery Ideas

## Background

Value was shrunk from 24 → 16 bytes. To fit, heap-backed variants now use thin
pointers (single `Arc<T>` or `Arc<DynObject>`). This introduced extra layers of
indirection that hurt hot paths. Render recovered to ~31ns but there is headroom.

---

## 🔴 CRITICAL: Triple indirection on Object access

`from_object` does this today:

```rust
Arc::new(DynObject::new(Arc::new(value)))
//  ^alloc 2           ^alloc 1
```

Access chain: `Arc` → `DynObject{ptr, vtable}` → `Arc<T>` → `T`

Three pointer chases on **every** attribute lookup, method call, iteration step.
Objects are the most common dynamic type (loops, context maps that aren't StrMap,
user objects). This is the single biggest regression vector.

DynObject already manages its own refcount through vtable (`__incref`/`__decref`).
The outer `Arc` is redundant — it only exists because DynObject is 16 bytes
(ptr + vtable) and doesn't fit in the 15-byte enum payload.

### Fix: make DynObject 8 bytes (one thin pointer)

Move the vtable pointer into the allocation header:

```
Allocation layout:  [vtable_ptr | refcount | T data ...]
                     ^--- DynObject stores pointer here
```

DynObject becomes a single `*const ()`. To call a method: read vtable from
`*(ptr as *const *const VTable)`, then dispatch. One alloc, one deref.

This is the same approach that Rust trait objects use internally, just manual.
The `type_erase!` macro is already fully unsafe so the complexity cost is low.

**Expected impact**: eliminates 1 allocation + 2 pointer chases per object
creation/access. Huge win for loop iteration (LoopState), context lookups,
and user-defined objects.

---

## 🔴 CRITICAL: Double indirection on strings

```rust
String(Arc<String>, StringType)
//     ^alloc 1  ^--- String does its own heap alloc (alloc 2)
```

Access chain: `Arc` → `String{ptr, len, cap}` → `[u8]`

Two pointer chases to read string data. Strings are the most common value type
in typical templates.

### Fix: thin arc string

Replace `Arc<String>` with a custom thin pointer to a single allocation:

```
Allocation layout:  [refcount | len | string bytes ...]
ThinArcStr:         *const ()   (8 bytes, thin)
```

One alloc, one deref to reach bytes. `StringType` tag still fits in remaining
payload bytes.

**Expected impact**: every string read (emit, filter, comparison) saves one
pointer chase. High frequency in render-heavy benchmarks.

---

## 🟡 MODERATE: Double indirection on Seq and Bytes

Same pattern as strings:

```rust
Seq(Arc<Vec<Value>>)   // Arc → Vec → [Value]
Bytes(Arc<Vec<u8>>)    // Arc → Vec → [u8]
```

### Fix: thin arc slice

```
Allocation layout:  [refcount | len | elements ...]
ThinArcSlice<T>:    *const ()   (8 bytes, thin)
```

Can share implementation with ThinArcStr. Note: Seq and Bytes are less hot than
Object/String in most templates, but Seq matters for large list iteration.

---

## 🟡 MODERATE: StrMap could use a simpler backing store

```rust
StrMap(Arc<BTreeMap<Arc<str>, Value>>)
```

For small maps (typical context! output: 2–8 keys), BTreeMap is overkill.
A sorted `Vec<(Arc<str>, Value)>` with binary search or even linear scan
would be faster and more cache-friendly.

Not as urgent since StrMap was added specifically to avoid the DynObject path,
and it's already a win. But worth revisiting after the critical fixes.

---

## Implementation strategy

### Recommended: unified ThinArc primitive

Build one `ThinArc<H>` type that backs everything:

```rust
struct ThinArc<H> {
    ptr: NonNull<()>,  // 8 bytes
    // points to: [refcount: AtomicUsize | header: H | data ...]
}
```

- **ThinArcStr** = `ThinArc<StringHeader>` where header has len + StringType
- **ThinArcSlice<T>** = `ThinArc<SliceHeader>` where header has len
- **DynObject** = `ThinArc<VTablePtr>` where header is the vtable pointer

Servo's `triomphe` crate has prior art. Could vendor or reference.

### Phased rollout

1. **Phase 1**: Fix DynObject (8-byte thin pointer, remove outer Arc). Benchmark.
2. **Phase 2**: Fix strings (ThinArcStr). Benchmark.
3. **Phase 3**: Fix Seq/Bytes (ThinArcSlice). Benchmark.
4. **Phase 4**: Consider StrMap backing store change.

### What to benchmark

- Current `all_elements` render benchmark (general)
- Object-heavy template (many `obj.attr` lookups) — will show Phase 1 wins
- String-heavy template (many `{{ var }}` emissions) — will show Phase 2 wins
- Large list iteration — will show Phase 3 wins
- Allocation counts per render (`dhat` or similar) — quantifies alloc reduction

---

## Not worth pursuing now

- **NaN-boxing / 8-byte Value**: High complexity, uncertain payoff, large unsafe surface
- **SmallStr capacity changes**: 14 bytes is fine, ±1 byte won't matter
- **Arena allocation**: Interesting but orthogonal to the indirection problem
- **Inline `#[inline(always)]` tuning**: Already explored extensively in autoresearch runs, diminishing returns
