use std::collections::BTreeSet;
use std::sync::Arc;

use minijinja::value::{StructObject, Value};
use minijinja::{context, Environment};

/// A struct that looks up from multiple values.
struct MergeContext(Vec<Value>);

impl StructObject for MergeContext {
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
        let mut rv = BTreeSet::new();
        for val in &self.0 {
            if let Ok(iter) = val.try_iter() {
                for item in iter {
                    if let Some(s) = item.as_str() {
                        if !rv.contains(s) {
                            rv.insert(Arc::from(s.to_string()));
                        }
                    }
                }
            }
        }
        rv.into_iter().collect()
    }
}

/// Merges one or more contexts.
pub fn merge_contexts<I>(i: I) -> Value
where
    I: Iterator<Item = Value>,
{
    Value::from_struct_object(MergeContext(i.into_iter().collect()))
}

fn main() {
    let env = Environment::new();
    let ctx = merge_contexts([context! { a => "A" }, context! { b => "B" }].into_iter());
    println!(
        "{}",
        env.render_str("Two variables: {{ a }} and {{ b }}!", ctx)
            .unwrap()
    );
}
