use std::collections::BTreeMap;
use std::sync::Arc;

use crate::value::Value;

pub(crate) type ClosureId = usize;

/// Values enclosed by macros declared in one frame.
#[derive(Debug, Default)]
pub(crate) struct Closure {
    values: BTreeMap<Arc<str>, Value>,
}

impl Closure {
    pub fn get(&self, key: &str) -> Option<Value> {
        self.values.get(key).cloned()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(|key| key.as_ref())
    }

    pub fn store(&mut self, key: &str, value: Value) {
        self.values.insert(Arc::from(key), value);
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
}
