use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::value::{StructObject, Value};

/// This object exists for the `namespace` function.
///
/// It's special in that it behaves like a dictionary in many ways but it's the only
/// object that can be used with `{% set %}` assignments.  This is used internally
/// in the vm via downcasting.
#[derive(Default)]
pub(crate) struct Namespace {
    data: Mutex<BTreeMap<Arc<str>, Value>>,
}

impl StructObject for Namespace {
    fn get_field(&self, name: &str) -> Option<Value> {
        self.data.lock().unwrap().get(name).cloned()
    }

    fn fields(&self) -> Vec<Arc<str>> {
        self.data.lock().unwrap().keys().cloned().collect()
    }

    fn field_count(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}

impl Namespace {
    pub(crate) fn set_field(&self, key: &str, value: Value) {
        self.data.lock().unwrap().insert(key.into(), value);
    }
}
