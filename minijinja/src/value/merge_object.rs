use std::collections::BTreeSet;
use std::sync::Arc;

use crate::value::object::MapObject;
use crate::value::Value;

/// Utility struct used by [`context!`](crate::context) to merge
/// multiple values.
#[derive(Clone)]
pub struct MergeObject(pub Vec<Value>);

impl MapObject for MergeObject {
    fn get_field(self: &Arc<Self>, field: &Value) -> Option<Value> {
        for val in &self.0 {
            if let Some(key) = field.as_str() {
                match val.get_attr(key) {
                    Ok(val) if !val.is_undefined() => return Some(val),
                    _ => {}
                }
            }
        }
        None
    }

    fn fields(self: &Arc<Self>) -> Vec<Value> {
        let mut seen = BTreeSet::new();
        let mut rv = Vec::new();
        for val in &self.0 {
            if let Ok(iter) = val.try_iter() {
                for item in iter {
                    let s: Result<Arc<str>, _> = item.try_into();
                    if let Ok(s) = s {
                        if !seen.contains(&s) {
                            seen.insert(s.clone());
                            rv.push(s.into());
                        }
                    }
                }
            }
        }
        rv
    }
}
