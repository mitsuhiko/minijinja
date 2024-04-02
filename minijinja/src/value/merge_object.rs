use std::collections::BTreeSet;
use std::sync::Arc;

use crate::value::{Enumerator, Object, ObjectExt, Value};

/// Utility struct used by [`context!`](crate::context) to merge
/// multiple values.
#[derive(Clone, Debug)]
pub struct MergeObject(pub Vec<Value>);

impl Object for MergeObject {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.0
            .iter()
            .filter_map(|x| x.get_item_opt(key))
            .find(|x| !x.is_undefined())
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        self.mapped_enumerator(|this| {
            let mut seen = BTreeSet::new();
            Box::new(
                this.0
                    .iter()
                    .flat_map(|v| v.try_iter().ok())
                    .flatten()
                    .filter_map(move |v| {
                        if seen.contains(&v) {
                            None
                        } else {
                            seen.insert(v.clone());
                            Some(v)
                        }
                    }),
            )
        })
    }
}
