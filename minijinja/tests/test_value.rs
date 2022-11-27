use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use insta::assert_snapshot;
use minijinja::value::{Object, ObjectKind, SeqObject, StructObject, Value};

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
fn test_map_object_iteration_and_indexing() {
    #[derive(Debug, Clone)]
    struct Point(i32, i32, i32);

    impl fmt::Display for Point {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}, {}, {}", self.0, self.1, self.2)
        }
    }

    impl Object for Point {
        fn kind(&self) -> ObjectKind<'_> {
            ObjectKind::Struct(self)
        }
    }

    impl StructObject for Point {
        fn get_field(&self, name: &str) -> Option<Value> {
            match name {
                "x" => Some(Value::from(self.0)),
                "y" => Some(Value::from(self.1)),
                "z" => Some(Value::from(self.2)),
                _ => None,
            }
        }

        fn static_fields(&self) -> Option<&'static [&'static str]> {
            Some(&["x", "y", "z"][..])
        }
    }

    let rv = minijinja::render!(
        "{% for key in point %}{{ key }}: {{ point[key] }}\n{% endfor %}",
        point => Value::from_object(Point(1, 2, 3))
    );
    assert_snapshot!(rv, @r###"
    x: 1
    y: 2
    z: 3
    "###);

    let rv = minijinja::render!(
        "{{ [point.x, point.z, point.missing_attribute] }}",
        point => Value::from_object(Point(1, 2, 3))
    );
    assert_snapshot!(rv, @r###"[1, 3, Undefined]"###);
}

#[test]
fn test_seq_object_iteration_and_indexing() {
    #[derive(Debug, Clone)]
    struct Point(i32, i32, i32);

    impl fmt::Display for Point {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}, {}, {}", self.0, self.1, self.2)
        }
    }

    impl Object for Point {
        fn kind(&self) -> ObjectKind<'_> {
            ObjectKind::Seq(self)
        }
    }

    impl SeqObject for Point {
        fn get_item(&self, index: usize) -> Option<Value> {
            match index {
                0 => Some(Value::from(self.0)),
                1 => Some(Value::from(self.1)),
                2 => Some(Value::from(self.2)),
                _ => None,
            }
        }

        fn item_count(&self) -> usize {
            3
        }
    }

    let rv = minijinja::render!(
        "{% for value in point %}{{ loop.index0 }}: {{ value }}\n{% endfor %}",
        point => Value::from_object(Point(1, 2, 3))
    );
    assert_snapshot!(rv, @r###"
    0: 1
    1: 2
    2: 3
    "###);

    let rv = minijinja::render!(
        "{{ [point[0], point[2], point[42]] }}",
        point => Value::from_object(Point(1, 2, 3))
    );
    assert_snapshot!(rv, @r###"[1, 3, Undefined]"###);
}

#[test]
fn test_value_string_interop() {
    let s = Arc::new(String::from("Hello"));
    let v = Value::from(s);
    assert_eq!(v.as_str(), Some("Hello"));
}
