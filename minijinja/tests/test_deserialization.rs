#![cfg(feature = "deserialization")]
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use similar_asserts::assert_eq;

use minijinja::value::{SeqObject, StructObject, Value};

#[test]
fn test_seq() {
    let v = Vec::<i32>::deserialize(Value::from(vec![1, 2, 3])).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn test_seq_object() {
    struct X;

    impl SeqObject for X {
        fn get_item(&self, idx: usize) -> Option<Value> {
            if idx < 3 {
                Some(Value::from(idx + 1))
            } else {
                None
            }
        }

        fn item_count(&self) -> usize {
            3
        }
    }

    let v = Vec::<i32>::deserialize(Value::from_seq_object(X)).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn test_map() {
    let v = BTreeMap::<String, i32>::deserialize(Value::from_iter([
        ("foo", Value::from(1)),
        ("bar", Value::from(2)),
    ]))
    .unwrap();
    assert_eq!(
        v,
        BTreeMap::from_iter([("foo".to_string(), 1), ("bar".to_string(), 2)])
    );
}

#[test]
fn test_struct_object() {
    struct X;

    impl StructObject for X {
        fn get_field(&self, name: &str) -> Option<Value> {
            match name {
                "a" => Some(Value::from(1)),
                "b" => Some(Value::from(2)),
                _ => None,
            }
        }
        fn static_fields(&self) -> Option<&'static [&'static str]> {
            Some(&["a", "b"])
        }
    }

    let v = BTreeMap::<String, i32>::deserialize(Value::from_struct_object(X)).unwrap();
    assert_eq!(
        v,
        BTreeMap::from_iter([("a".to_string(), 1), ("b".to_string(), 2)])
    );
}

#[test]
fn test_basics() {
    assert_eq!(bool::deserialize(Value::from(true)).unwrap(), true);
    assert_eq!(bool::deserialize(Value::from(false)).unwrap(), false);
    assert_eq!(f32::deserialize(Value::from(1.0)).unwrap(), 1.0);
    assert_eq!(i32::deserialize(Value::from(2)).unwrap(), 2);
    assert_eq!(String::deserialize(Value::from("foo")).unwrap(), "foo");
    assert_eq!(Option::<i32>::deserialize(Value::from(2)).unwrap(), Some(2));
    assert_eq!(Option::<i32>::deserialize(Value::from(())).unwrap(), None);
}

#[test]
fn test_enum() {
    #[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
    enum Foo {
        A,
        B,
        C(i32),
    }

    assert_eq!(Foo::deserialize(Value::from("A")).unwrap(), Foo::A);
    assert_eq!(Foo::deserialize(Value::from("B")).unwrap(), Foo::B);
    assert_eq!(
        Foo::deserialize(Value::from(BTreeMap::from_iter([("C", 42)]))).unwrap(),
        Foo::C(42)
    );
}

#[test]
fn test_invalid() {
    struct X;

    impl Serialize for X {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("meh"))
        }
    }

    let v = Value::from_serializable(&X);
    assert_eq!(v.to_string(), "<invalid value: meh>");

    let err = bool::deserialize(v).unwrap_err();
    assert_eq!(err.to_string(), "cannot deserialize: meh");
}
