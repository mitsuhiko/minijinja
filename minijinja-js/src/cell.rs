use std::fmt;
use std::ops::Deref;

// MiniJinja requires callbacks and objects to be `Send + Sync`.  JavaScript
// handles are neither, but on `wasm32-unknown-unknown` without the atomics
// target feature there is exactly one thread, so no value can ever be moved
// to or shared with another thread.
#[cfg(target_feature = "atomics")]
compile_error!("minijinja-js does not support wasm threads (the atomics target feature)");

/// Wraps a JavaScript handle so that it can be stored in MiniJinja values.
pub struct JsCell<T>(T);

// SAFETY: see the module comment above.  Without the atomics target feature
// a wasm module cannot spawn threads, so there is only ever a single thread
// that can observe this value.
unsafe impl<T> Send for JsCell<T> {}
unsafe impl<T> Sync for JsCell<T> {}

impl<T> JsCell<T> {
    pub fn new(value: T) -> JsCell<T> {
        JsCell(value)
    }
}

impl<T> Deref for JsCell<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: fmt::Debug> fmt::Debug for JsCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
