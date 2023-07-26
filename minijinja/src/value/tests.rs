use std::sync::Arc;
use std::fmt;

use crate::value::*;

use similar_asserts::assert_eq;

#[test]
fn test_dynamic_object_roundtrip() {
    use std::sync::atomic::{self, AtomicUsize};

    #[derive(Debug, Clone)]
    struct X(Arc<AtomicUsize>);

    impl fmt::Display for X {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0.load(atomic::Ordering::Relaxed))
        }
    }

    impl Object for X {
        fn value<'a>(&'a self) -> Value<'a> {
            Value::from_map_ref(self)
        }
    }

    impl crate::value::object::MapObject for X {
        fn get_field(&self, key: &ValueBox) -> Option<ValueBox> {
            match key.as_str() {
                Some("value") => Some(ValueBox::from(self.0.load(atomic::Ordering::Relaxed))),
                _ => None,
            }
        }

        fn static_fields(&self) -> Option<&'static [&'static str]> {
            Some(&["value"][..])
        }
    }

    let x = X(Default::default());
    let x_value = Value::from_map_object(x.clone());
    x.0.fetch_add(42, atomic::Ordering::Relaxed);
    let x_clone = ValueBox::from_serializable(&x_value);
    x.0.fetch_add(23, atomic::Ordering::Relaxed);

    assert_eq!(x_value.get_attr("value").unwrap().to_string(), "65");
    assert_eq!(x_clone.get_attr("value").unwrap().to_string(), "65");
}

#[test]
fn test_string_char() {
    let val = ValueBox::from('a');
    assert_eq!(char::try_from(val).unwrap(), 'a');
    let val = ValueBox::from("a");
    assert_eq!(char::try_from(val).unwrap(), 'a');
    let val = ValueBox::from("wat");
    assert!(char::try_from(val).is_err());
}

#[test]
#[cfg(target_pointer_width = "64")]
fn test_sizes() {
    // assert_eq!(std::mem::size_of::<ValueBox>(), 32);
    // TODO: todo!()
}
