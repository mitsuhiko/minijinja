Update `minijinja-cabi/include/minijinja.h` so it matches the Rust C ABI in `minijinja-cabi/src/`.

## Goal

Keep the C header in sync with exported Rust FFI symbols **without** using code generation.

## Source of Truth

Read these files first:

- `minijinja-cabi/src/lib.rs`
- `minijinja-cabi/src/env.rs`
- `minijinja-cabi/src/error.rs`
- `minijinja-cabi/src/value.rs`
- `minijinja-cabi/src/macros.rs`
- `minijinja-cabi/src/utils.rs`

Treat Rust code as authoritative.

## What to sync

1. All exported functions:
   - `#[no_mangle] pub extern "C" fn ...`
   - `ffi_fn! { unsafe fn ... }` declarations
2. All public C ABI types used by exported functions:
   - `#[repr(C)]` enums/structs
   - public opaque structs
   - callback typedefs
3. Signatures, constness, pointer mutability, and integer widths.
4. Public API docs/comments (short form is fine, but keep them accurate).

## Header conventions to preserve

- Keep include guard: `_minijinja_h_included`
- Keep `#pragma once`
- Keep includes: `<stdint.h>`, `<stddef.h>`, `<stdbool.h>`
- Keep `MINIJINJA_API` macro block
- Keep C++ `extern "C"` guard
- Keep `mj_value` layout exactly:
  - `uint64_t _opaque[3];`
- Keep function declarations sorted by function name
- Do **not** add any “generated/auto-generated” note

## Rust → C type mapping reminders

- `usize` ↔ `uintptr_t`
- `u32` ↔ `uint32_t`
- `u64` ↔ `uint64_t`
- `i32` ↔ `int32_t`
- `i64` ↔ `int64_t`
- `f32` ↔ `float`
- `f64` ↔ `double`
- `*const c_char` ↔ `const char *`
- `*mut c_char` ↔ `char *`

## Verification checklist

- Every exported Rust symbol has a declaration in `include/minijinja.h`
- No stale declarations remain in the header
- Enum variants in header match Rust order/names
- `cargo check -p minijinja-cabi` passes

When done, provide a brief summary of what changed in the header.
