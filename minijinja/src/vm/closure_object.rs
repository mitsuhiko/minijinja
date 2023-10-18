use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::value::{Object, ObjectKind, StructObject, Value};

/// Closure cycle breaker utility.
///
/// The closure tracker is a crude way to forcefully break the cycles
/// caused by closures on teardown of the state.  Whenever a closure is
/// exposed to the engine it's also passed to the [`track_closure`] function.
#[derive(Default)]
pub struct ClosureTracker {
    closures: Mutex<BTreeMap<usize, Arc<Closure>>>,
}

impl ClosureTracker {
    /// This accepts a closure as value and registers it in the
    /// tracker for cycle breaking.
    ///
    /// This only registers each closure once.
    pub(crate) fn track_closure(&self, closure: Arc<Closure>) {
        let ptr = &closure as &Closure;
        let id = ptr as *const _ as usize;
        self.closures.lock().unwrap().insert(id, closure);
    }
}

impl Drop for ClosureTracker {
    fn drop(&mut self) {
        for closure in self.closures.lock().unwrap().values() {
            closure.clear();
        }
    }
}

/// Utility to enclose values for macros.
///
/// See `closure` on the [`Frame`] for how it's used.
#[derive(Debug, Default)]
pub(crate) struct Closure {
    values: Arc<Mutex<BTreeMap<Arc<str>, Value>>>,
}

impl Closure {
    /// Stores a value by key in the closure.
    pub fn store(&self, key: &str, value: Value) {
        self.values.lock().unwrap().insert(Arc::from(key), value);
    }

    /// Upset a value into the closure.
    #[cfg(feature = "macros")]
    pub fn store_if_missing<F: FnOnce() -> Value>(&self, key: &str, f: F) {
        let mut values = self.values.lock().unwrap();
        if !values.contains_key(key) {
            values.insert(Arc::from(key), f());
        }
    }

    /// Clears the closure.
    ///
    /// This is required to break cycles.
    pub fn clear(&self) {
        self.values.lock().unwrap().clear();
    }
}

impl fmt::Display for Closure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        for (key, value) in self.values.lock().unwrap().iter() {
            m.entry(&key, &value);
        }
        m.finish()
    }
}

impl Object for Closure {
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Struct(self)
    }
}

impl StructObject for Closure {
    fn fields(&self) -> Vec<Arc<str>> {
        self.values.lock().unwrap().keys().cloned().collect()
    }

    fn get_field(&self, name: &str) -> Option<Value> {
        self.values.lock().unwrap().get(name).cloned()
    }
}
