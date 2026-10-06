//! Cheap access to the Python token from within the engine.
//!
//! The engine calls back into the binding through the `Object` trait and
//! various callbacks which do not carry a [`Python`] token.  The obvious way
//! to get one is [`Python::attach`] but even when the thread is already
//! attached that is not free: PyO3 takes a process wide lock on every call to
//! process deferred reference count changes.  As templates do this for every
//! single attribute lookup, that lock shows up in profiles and on free-threaded
//! Python it turns into a point of contention between all rendering threads.
//!
//! Since we never detach from the interpreter while rendering, we can instead
//! remember that the thread is attached for the duration of a render and hand
//! out the token directly.
use std::cell::Cell;

use pyo3::Python;

thread_local! {
    static ATTACHED: Cell<bool> = const { Cell::new(false) };
}

/// Marks the current thread as attached for as long as the scope is alive.
///
/// Within the scope the thread must not detach from the interpreter (that
/// is, no [`Python::detach`]) other than from within Python code that
/// re-attaches before it returns.
pub struct AttachedScope {
    was_attached: bool,
    // the scope is bound to the current thread
    _marker: std::marker::PhantomData<*const ()>,
}

impl AttachedScope {
    pub fn enter(_py: Python<'_>) -> AttachedScope {
        AttachedScope {
            was_attached: ATTACHED.replace(true),
            _marker: std::marker::PhantomData,
        }
    }
}

impl Drop for AttachedScope {
    fn drop(&mut self) {
        ATTACHED.set(self.was_attached);
    }
}

/// Invokes the closure with the Python token.
///
/// Within an [`AttachedScope`] this is very cheap, outside it falls back to
/// [`Python::attach`].
#[inline]
pub fn with_py<F, R>(f: F) -> R
where
    F: for<'py> FnOnce(Python<'py>) -> R,
{
    if ATTACHED.get() {
        // SAFETY: an `AttachedScope` can only be created with a Python token
        // and the binding never detaches while it's alive.
        f(unsafe { Python::assume_attached() })
    } else {
        Python::attach(f)
    }
}
