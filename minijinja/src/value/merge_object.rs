use std::collections::BTreeSet;
use std::sync::Arc;

use crate::value::object::StructObject;
use crate::value::Value;

/// Utility struct used by [`context!`](crate::context) to merge
/// multiple values.
pub struct MergeObject(pub Vec<Value>);

impl StructObject for MergeObject {
    fn get_field(&self, field: &str) -> Option<Value> {
        for val in &self.0 {
            match val.get_attr(field) {
                Ok(val) if !val.is_undefined() => return Some(val),
                _ => {}
            }
        }
        None
    }

    fn fields(&self) -> Vec<Arc<str>> {
        let mut seen = BTreeSet::new();
        let mut rv = Vec::new();
        for val in &self.0 {
            if let Ok(iter) = val.try_iter() {
                for item in iter {
                    let s: Result<Arc<str>, _> = item.try_into();
                    if let Ok(s) = s {
                        if !seen.contains(&s) {
                            seen.insert(s.clone());
                            rv.push(s);
                        }
                    }
                }
            }
        }
        rv
    }
}
