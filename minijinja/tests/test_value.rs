use std::cmp::Ordering;
use std::fmt;

use insta::assert_snapshot;
use minijinja::value::{Object, Value};
use minijinja::ErrorKind;

#[test]
fn test_sort() {
    let mut v = vec![
        Value::from(100u64),
        Value::from(80u32),
        Value::from(30i16),
        Value::from(true),
        Value::from(false),
        Value::from(99i128),
        Value::from(1000f32),
    ];
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    insta::assert_debug_snapshot!(&v, @r###"
    [
        false,
        true,
        30,
        80,
        99,
        100,
        1000.0,
    ]
    "###);
}

#[test]
fn test_safe_string_roundtrip() {
    let v = Value::from_safe_string("<b>HTML</b>".into());
    let v2 = Value::from_serializable(&v);
    assert!(v.is_safe());
    assert!(v2.is_safe());
    assert_eq!(v.to_string(), v2.to_string());
}

#[test]
fn test_undefined_roundtrip() {
    let v = Value::UNDEFINED;
    let v2 = Value::from_serializable(&v);
    assert!(v.is_undefined());
    assert!(v2.is_undefined());
}

#[test]
fn test_value_serialization() {
    // make sure if we serialize to json we get regular values
    assert_eq!(serde_json::to_string(&Value::UNDEFINED).unwrap(), "null");
    assert_eq!(
        serde_json::to_string(&Value::from_safe_string("foo".to_string())).unwrap(),
        "\"foo\""
    );
}

#[test]
fn test_float_to_string() {
    assert_eq!(Value::from(42.4242f64).to_string(), "42.4242");
    assert_eq!(Value::from(42.0f32).to_string(), "42.0");
}

#[test]
fn test_value_as_slice() {
    let val = Value::from(vec![1u32, 2, 3]);
    assert_eq!(
        val.as_slice().unwrap(),
        &[Value::from(1), Value::from(2), Value::from(3)]
    );
    assert_eq!(Value::UNDEFINED.as_slice().unwrap(), &[]);
    assert_eq!(Value::from(()).as_slice().unwrap(), &[]);
    assert_eq!(
        Value::from("foo").as_slice().unwrap_err().kind(),
        ErrorKind::InvalidOperation
    );
}

#[test]
fn test_value_as_bytes() {
    assert_eq!(Value::from("foo").as_bytes(), Some(&b"foo"[..]));
    assert_eq!(Value::from(&b"foo"[..]).as_bytes(), Some(&b"foo"[..]));
}

#[test]
fn test_value_by_index() {
    let val = Value::from(vec![1u32, 2, 3]);
    assert_eq!(val.get_item_by_index(0).unwrap(), Value::from(1));
    assert!(val.get_item_by_index(4).unwrap().is_undefined());
}

#[test]
fn test_object_iteration() {
    #[derive(Debug, Clone)]
    struct Point(i32, i32, i32);

    impl fmt::Display for Point {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}, {}, {}", self.0, self.1, self.2)
        }
    }

    impl Object for Point {
        fn get_attr(&self, name: &str) -> Option<Value> {
            match name {
                "x" => Some(Value::from(self.0)),
                "y" => Some(Value::from(self.1)),
                "z" => Some(Value::from(self.2)),
                _ => None,
            }
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = &str> + '_> {
            Box::new(["x", "y", "z"].into_iter())
        }
    }

    let point = Point(1, 2, 3);
    let rv = minijinja::render!(
        "{% for key in point %}{{ key }}: {{ point[key] }}\n{% endfor %}",
        point => Value::from_object(point)
    );
    assert_snapshot!(rv, @r###"
    x: 1
    y: 2
    z: 3
    "###);
}
