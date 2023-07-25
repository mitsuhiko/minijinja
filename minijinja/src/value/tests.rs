use std::sync::Arc;
use std::fmt;

use crate::value::*;

use similar_asserts::assert_eq;

#[test]
fn test_dynamic_object_roundtrip() {
    use std::sync::atomic::{self, AtomicUsize};

    #[derive(Debug)]
    struct X(AtomicUsize);

    impl fmt::Display for X {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0.load(atomic::Ordering::Relaxed))
        }
    }

    impl Object for X {
        fn kind(&self) -> ObjectKind<'_> {
            ObjectKind::Struct(self)
        }
    }

    impl crate::value::object::StructObject for X {
        fn get_field(&self, key: &Value) -> Option<Value> {
            match key.as_str() {
                Some("value") => Some(Value::from(self.0.load(atomic::Ordering::Relaxed))),
                _ => None,
            }
        }

        fn static_fields(&self) -> Option<&'static [&'static str]> {
            Some(&["value"][..])
        }
    }

    let x = Arc::new(X(Default::default()));
    let x_value = Value::from(x.clone());
    x.0.fetch_add(42, atomic::Ordering::Relaxed);
    let x_clone = Value::from_serializable(&x_value);
    x.0.fetch_add(23, atomic::Ordering::Relaxed);

    assert_eq!(x_value.to_string(), "65");
    assert_eq!(x_clone.to_string(), "65");
}

#[test]
fn test_string_char() {
    let val = Value::from('a');
    assert_eq!(char::try_from(val).unwrap(), 'a');
    let val = Value::from("a");
    assert_eq!(char::try_from(val).unwrap(), 'a');
    let val = Value::from("wat");
    assert!(char::try_from(val).is_err());
}

#[test]
#[cfg(target_pointer_width = "64")]
fn test_sizes() {
    assert_eq!(std::mem::size_of::<Value>(), 24);
}
