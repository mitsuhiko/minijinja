use std::collections::BTreeMap;

use crate::value::Value;

pub(crate) type ClosureId = usize;

/// Values enclosed by macros declared in one frame.
#[derive(Debug, Default)]
pub(crate) struct Closure<'env> {
    values: BTreeMap<&'env str, Value>,
}

impl<'env> Closure<'env> {
    pub fn get(&self, key: &str) -> Option<Value> {
        self.values.get(key).cloned()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().copied()
    }

    pub fn store(&mut self, key: &'env str, value: Value) {
        self.values.insert(key, value);
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
}
