use std::collections::BTreeSet;
use std::sync::Arc;

use crate::value::{Enumerator, Object, Value, ValueKind};

#[derive(Clone, Debug)]
struct MergeObject(pub Box<[Value]>);

impl Object for MergeObject {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.0
            .iter()
            .filter_map(|x| x.get_item_opt(key))
            .find(|x| !x.is_undefined())
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        // we collect here the whole internal object once on iteration so that
        // we have an enumerator with a known length.
        let items = self
            .0
            .iter()
            .filter(|x| x.kind() == ValueKind::Map)
            .flat_map(|v| v.try_iter().ok())
            .flatten()
            .collect::<BTreeSet<_>>();
        Enumerator::Iter(Box::new(items.into_iter()))
    }
}

/// Utility function to merge multiple maps into a single one.
///
/// If values are passed that are not maps, they are for the most part ignored.
/// They cannot be enumerated, but attribute lookups can still work.   That's
/// because [`get_value`](crate::value::Object::get_value) is forwarded through
/// to all objects.
///
/// This is the operation the [`context!`](crate::context) macro uses behind
/// the scenes.  The merge is done lazily which means that any dynamic object
/// that behaves like a map can be used here.
///
/// ```
/// use minijinja::{context, value::merge_maps};
///
/// let ctx1 = context!{
///     name => "John",
///     age => 30
/// };
///
/// let ctx2 = context!{
///     location => "New York",
///     age => 25  // This will be overridden by ctx1's value
/// };
///
/// let merged = merge_maps([ctx1, ctx2]);
/// ```
pub fn merge_maps<I, V>(iter: I) -> Value
where
    I: IntoIterator<Item = V>,
    V: Into<Value>,
{
    Value::from_object(MergeObject(iter.into_iter().map(Into::into).collect()))
}

#[test]
fn test_merge_object() {
    use std::collections::BTreeMap;

    let o = merge_maps([Value::from("abc"), Value::from(vec![1, 2, 3])]);
    assert_eq!(o, Value::from(BTreeMap::<String, String>::new()));

    let mut map1 = BTreeMap::new();
    map1.insert("a", 1);
    map1.insert("b", 2);

    let mut map2 = BTreeMap::new();
    map2.insert("b", 3);
    map2.insert("c", 4);

    let merged = merge_maps([Value::from(map1), Value::from(map2)]);

    // Check that the merged object contains all keys with expected values
    // The value from the first map should be used when keys overlap
    assert_eq!(merged.get_attr("a").unwrap(), Value::from(1));
    assert_eq!(merged.get_attr("b").unwrap(), Value::from(2)); // Takes value from map1
    assert_eq!(merged.get_attr("c").unwrap(), Value::from(4));
}
