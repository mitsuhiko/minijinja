use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::value::{Enumeration, Object, Value};

/// This object exists for the `namespace` function.
///
/// It's special in that it behaves like a dictionary in many ways but it's the only
/// object that can be used with `{% set %}` assignments.  This is used internally
/// in the vm via downcasting.
#[derive(Debug, Default)]
pub(crate) struct Namespace {
    data: Mutex<BTreeMap<Arc<str>, Value>>,
}

impl Object for Namespace {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.data.lock().unwrap().get(key.as_str()?).cloned()
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        let data = self.data.lock().unwrap();
        let keys = data.keys().cloned().map(Value::from);
        Enumeration::Values(keys.collect())
    }
}

impl Namespace {
    pub(crate) fn set_field(&self, key: &str, value: Value) {
        self.data.lock().unwrap().insert(key.into(), value);
    }
}
