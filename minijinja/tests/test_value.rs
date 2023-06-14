use std::fmt;
use std::sync::Arc;

use insta::assert_snapshot;
use similar_asserts::assert_eq;

use minijinja::value::{Kwargs, Object, ObjectKind, Rest, SeqObject, StructObject, Value};
use minijinja::{Environment, Error};

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
    v.sort();
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
fn test_builtin_seq_objects() {
    let rv = minijinja::render!(
        "{{ val }}",
        val => Value::from_seq_object(vec![true, false]),
    );
    assert_snapshot!(rv, @r###"[true, false]"###);

    let rv = minijinja::render!(
        "{{ val }}",
        val => Value::from_seq_object(&["foo", "bar"][..]),
    );
    assert_snapshot!(rv, @r###"["foo", "bar"]"###);
}

#[test]
fn test_value_string_interop() {
    let s = Arc::new(String::from("Hello"));
    let v = Value::from(s);
    assert_eq!(v.as_str(), Some("Hello"));
}

#[test]
fn test_value_object_interface() {
    let val = Value::from_seq_object(vec![1u32, 2, 3, 4]);
    let seq = val.as_seq().unwrap();
    assert_eq!(seq.item_count(), 4);

    let obj = val.as_object().unwrap();
    let seq2 = match obj.kind() {
        ObjectKind::Seq(s) => s,
        _ => panic!("did not expect this"),
    };
    assert_eq!(seq2.item_count(), 4);
    assert_eq!(obj.to_string(), "[1, 2, 3, 4]");
}

#[test]
fn test_obj_downcast() {
    #[derive(Debug)]
    struct Thing {
        id: usize,
    }

    impl fmt::Display for Thing {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Debug::fmt(self, f)
        }
    }

    impl Object for Thing {}

    let x_value = Value::from_object(Thing { id: 42 });
    let value_as_obj = x_value.as_object().unwrap();
    assert!(value_as_obj.is::<Thing>());
    let thing = value_as_obj.downcast_ref::<Thing>().unwrap();
    assert_eq!(thing.id, 42);
}

#[test]
fn test_value_cmp() {
    assert_eq!(Value::from(&[1][..]), Value::from(&[1][..]));
    assert_ne!(Value::from(&[1][..]), Value::from(&[2][..]));
    assert_eq!(Value::UNDEFINED, Value::UNDEFINED);
}

#[test]
fn test_call_kwargs() {
    let mut env = Environment::new();
    env.add_template("foo", "").unwrap();
    let tmpl = env.get_template("foo").unwrap();
    let state = tmpl.new_state();
    let val = Value::from_function(|kwargs: Kwargs| kwargs.get::<i32>("foo"));
    let rv = val
        .call(
            &state,
            &[Kwargs::from_iter([("foo", Value::from(42))]).into()],
        )
        .unwrap();
    assert_eq!(rv, Value::from(42));
}

#[test]
fn test_filter_basics() {
    fn test(a: u32, b: u32) -> Result<u32, Error> {
        Ok(a + b)
    }

    let mut env = Environment::new();
    env.add_filter("test", test);
    assert_eq!(
        env.empty_state()
            .apply_filter("test", &[Value::from(23), Value::from(42)])
            .unwrap(),
        Value::from(65)
    );
}

#[test]
fn test_rest_args() {
    fn sum(val: u32, rest: Rest<u32>) -> u32 {
        rest.iter().fold(val, |a, b| a + b)
    }

    let mut env = Environment::new();
    env.add_filter("sum", sum);
    assert_eq!(
        env.empty_state()
            .apply_filter(
                "sum",
                &[
                    Value::from(1),
                    Value::from(2),
                    Value::from(3),
                    Value::from(4)
                ][..]
            )
            .unwrap(),
        Value::from(1 + 2 + 3 + 4)
    );
}

#[test]
fn test_optional_args() {
    fn add(val: u32, a: u32, b: Option<u32>) -> Result<u32, Error> {
        // ensure we really get our value as first argument
        assert_eq!(val, 23);
        let mut sum = val + a;
        if let Some(b) = b {
            sum += b;
        }
        Ok(sum)
    }

    let mut env = crate::Environment::new();
    env.add_filter("add", add);
    let state = env.empty_state();
    assert_eq!(
        state
            .apply_filter("add", &[Value::from(23), Value::from(42)][..])
            .unwrap(),
        Value::from(65)
    );
    assert_eq!(
        state
            .apply_filter(
                "add",
                &[Value::from(23), Value::from(42), Value::UNDEFINED][..]
            )
            .unwrap(),
        Value::from(65)
    );
    assert_eq!(
        state
            .apply_filter(
                "add",
                &[Value::from(23), Value::from(42), Value::from(1)][..]
            )
            .unwrap(),
        Value::from(66)
    );
}

#[test]
fn test_values_in_vec() {
    fn upper(value: &str) -> String {
        value.to_uppercase()
    }

    fn sum(value: Vec<i64>) -> i64 {
        value.into_iter().sum::<i64>()
    }

    let mut env = Environment::new();
    env.add_filter("upper", upper);
    env.add_filter("sum", sum);
    let state = env.empty_state();

    assert_eq!(
        state
            .apply_filter("upper", &[Value::from("Hello World!")])
            .unwrap(),
        Value::from("HELLO WORLD!")
    );

    assert_eq!(
        state
            .apply_filter("sum", &[Value::from(vec![Value::from(1), Value::from(2)])])
            .unwrap(),
        Value::from(3)
    );
}

#[test]
fn test_seq_object_borrow() {
    fn connect(values: &dyn SeqObject) -> String {
        let mut rv = String::new();
        for item in values.iter() {
            rv.push_str(&item.to_string())
        }
        rv
    }

    let mut env = Environment::new();
    env.add_filter("connect", connect);
    let state = env.empty_state();
    assert_eq!(
        state
            .apply_filter(
                "connect",
                &[Value::from(vec![Value::from("HELLO"), Value::from(42)])]
            )
            .unwrap(),
        Value::from("HELLO42")
    );
}
