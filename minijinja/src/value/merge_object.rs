use std::collections::BTreeSet;
use std::sync::Arc;

use crate::value::{Enumeration, Object, ObjectExt, Value};

/// Utility struct used by [`context!`](crate::context) to merge
/// multiple values.
#[derive(Clone, Debug)]
pub struct MergeObject(pub Vec<Value>);

impl Object for MergeObject {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        for val in &self.0 {
            match val.get_item(key) {
                Ok(val) if !val.is_undefined() => return Some(val),
                _ => {}
            }
        }

        None
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        self.mapped_enumeration(|this| {
            let mut seen = BTreeSet::new();
            let iter = this
                .0
                .iter()
                .flat_map(|v| v.try_iter().ok())
                .flatten()
                .filter_map(move |v| {
                    if seen.contains(&v) {
                        return None;
                    }

                    seen.insert(v.clone());
                    Some(v)
                });

            Box::new(iter)
        })
    }
}
