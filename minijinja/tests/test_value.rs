use std::sync::Arc;

use insta::assert_snapshot;
use similar_asserts::assert_eq;

use minijinja::value::{DynObject, Enumerator, Kwargs, Object, ObjectRepr, Rest, Value};
use minijinja::{args, render, Environment, Error};

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
fn test_sort_different_types() {
    let mut v = vec![
        Value::from(100u64),
        Value::from("bar"),
        Value::from(1),
        Value::from_iter([1, 2]),
        Value::from(80u32),
        Value::from(30i16),
        Value::from_iter([("a", 3)]),
        Value::from_iter([("a", 2)]),
        Value::from_iter([("b", 0)]),
        Value::from_iter([("b", 3)]),
        Value::from_iter([0, 2]),
        Value::from(true),
        Value::UNDEFINED,
        Value::from("zzz"),
        Value::from(false),
        Value::from(-100),
        Value::from(-50.0f64),
        Value::from(-75.0f32),
        Value::from(99i128),
        Value::from(1000f32),
        Value::from_iter([0, 1]),
        Value::from_iter([1, 1]),
        Value::from("foo"),
        Value::from(()),
        Value::from(0),
        Value::from(-f64::INFINITY),
        Value::from(f64::NAN),
        Value::from(f64::INFINITY),
    ];
    v.sort();
    insta::assert_debug_snapshot!(&v, @r###"
    [
        undefined,
        none,
        -inf,
        -100,
        -75.0,
        -50.0,
        false,
        0,
        true,
        1,
        30,
        80,
        99,
        100,
        1000.0,
        inf,
        NaN,
        "bar",
        "foo",
        "zzz",
        [
            0,
            1,
        ],
        [
            0,
            2,
        ],
        [
            1,
            1,
        ],
        [
            1,
            2,
        ],
        {
            "a": 2,
        },
        {
            "a": 3,
        },
        {
            "b": 0,
        },
        {
            "b": 3,
        },
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

    impl Object for Point {
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str()? {
                "x" => Some(Value::from(self.0)),
                "y" => Some(Value::from(self.1)),
                "z" => Some(Value::from(self.2)),
                _ => None,
            }
        }

        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Str(&["x", "y", "z"])
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
    assert_snapshot!(rv, @"[1, 3, undefined]");
}

#[test]
fn test_seq_object_iteration_and_indexing() {
    #[derive(Debug, Clone)]
    struct Point(i32, i32, i32);

    impl Object for Point {
        fn repr(self: &Arc<Self>) -> ObjectRepr {
            ObjectRepr::Seq
        }

        fn get_value(self: &Arc<Self>, index: &Value) -> Option<Value> {
            match index.as_usize()? {
                0 => Some(Value::from(self.0)),
                1 => Some(Value::from(self.1)),
                2 => Some(Value::from(self.2)),
                _ => None,
            }
        }

        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Seq(3)
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
    assert_snapshot!(rv, @"[1, 3, undefined]");
}

#[test]
fn test_builtin_seq_objects() {
    let rv = minijinja::render!(
        "{{ val }}",
        val => Value::from_object(vec![true, false]),
    );
    assert_snapshot!(rv, @r###"[true, false]"###);

    let rv = minijinja::render!(
        "{{ val }}",
        val => Value::from_object(vec!["foo", "bar"]),
    );
    assert_snapshot!(rv, @r###"["foo", "bar"]"###);
}

#[test]
fn test_value_object_interface() {
    let val = Value::from_object(vec![1u32, 2, 3, 4]);
    let obj = val.as_object().unwrap();
    assert_eq!(obj.len(), Some(4));
    assert_eq!(obj.to_string(), "[1, 2, 3, 4]");
}

#[test]
fn test_obj_downcast() {
    #[derive(Debug)]
    struct Thing {
        id: usize,
    }

    impl Object for Thing {}

    let x_value = Value::from_object(Thing { id: 42 });
    let value_as_obj = x_value.as_object().unwrap();
    assert!(value_as_obj.is::<Thing>());
    let thing = value_as_obj.downcast::<Thing>().unwrap();
    assert_eq!(thing.id, 42);
}

#[test]
fn test_seq_object_downcast() {
    #[derive(Debug, Clone)]
    struct Thing {
        moo: i32,
    }

    impl Object for Thing {
        fn get_value(self: &Arc<Self>, idx: &Value) -> Option<Value> {
            if idx.as_usize()? < 3 {
                Some(idx.clone())
            } else {
                None
            }
        }

        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Seq(3)
        }
    }

    let obj = Value::from_object(Thing { moo: 42 });

    let seq = obj.downcast_object_ref::<Thing>().unwrap();
    assert_eq!(seq.moo, 42);
}

#[test]
fn test_struct_object_downcast() {
    #[derive(Debug, Clone)]
    struct Thing {
        moo: i32,
    }

    impl Object for Thing {}

    let obj = Value::from_object(Thing { moo: 42 });
    let seq = obj.downcast_object_ref::<Thing>().unwrap();
    assert_eq!(seq.moo, 42);
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
fn test_kwargs_error() {
    let kwargs = Kwargs::from_iter([("foo", Value::from(42))]);
    let bar = kwargs.get::<Value>("bar").unwrap_err();
    assert_eq!(bar.detail(), Some("missing keyword argument 'bar'"));
}

#[test]
fn test_return_none() {
    let env = Environment::empty();
    let val = Value::from_function(|| -> Result<(), Error> { Ok(()) });
    let rv = val.call(&env.empty_state(), &[][..]).unwrap();
    assert!(rv.is_none());
    let val = Value::from_function(|| ());
    let rv = val.call(&env.empty_state(), &[][..]).unwrap();
    assert!(rv.is_none());
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
            .apply_filter("test", args!(23, 42))
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
            .apply_filter("sum", args!(1, 2, 3, 4))
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
        state.apply_filter("add", args!(23, 42)).unwrap(),
        Value::from(65)
    );
    assert_eq!(
        state
            .apply_filter("add", args!(23, 42, Value::UNDEFINED))
            .unwrap(),
        Value::from(65)
    );
    assert_eq!(
        state.apply_filter("add", args!(23, 42, 1)).unwrap(),
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
        state.apply_filter("upper", args!("Hello World!")).unwrap(),
        Value::from("HELLO WORLD!")
    );

    assert_eq!(
        state.apply_filter("sum", args!(vec![1, 2])).unwrap(),
        Value::from(3)
    );
}

#[test]
fn test_seq_object_borrow() {
    fn connect(values: DynObject) -> String {
        let mut rv = String::new();
        for item in values.try_iter().into_iter().flatten() {
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
                args!(vec![Value::from("HELLO"), Value::from(42)])
            )
            .unwrap(),
        Value::from("HELLO42")
    );
}

#[test]
fn test_one_shot_iterator() {
    let value = Value::make_one_shot_iterator(0..10);
    assert_eq!(value.to_string(), "<iterator>");
    let rv = render!(
        "{% for item in iter %}[{{ item }}]{% endfor %}",
        iter => value
    );
    assert_snapshot!(rv, @"[0][1][2][3][4][5][6][7][8][9]");

    let rv = render!(
        "{% for item in iter %}- {{ item }}: {{ loop.index }} / {{ loop.length|default('?') }}\n{% endfor %}",
        iter => Value::make_one_shot_iterator('a'..'f')
    );
    assert_snapshot!(rv, @r###"
    - a: 1 / ?
    - b: 2 / ?
    - c: 3 / ?
    - d: 4 / ?
    - e: 5 / ?
    "###);

    let rv = render!(
        "{% for item in iter %}{{ item }}{% endfor %}{% for item in iter %}{{ item }}{% endfor %}",
        iter => Value::make_one_shot_iterator('a'..'f')
    );
    assert_snapshot!(rv, @r###"abcde"###);
}

#[test]
fn test_make_iterable() {
    let value = Value::make_iterable(|| 0..10);
    assert_eq!(value.to_string(), "[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]");
    let rv = render!(
        "{% for item in iter %}[{{ item }}]{% endfor %}",
        iter => value
    );
    assert_snapshot!(rv, @"[0][1][2][3][4][5][6][7][8][9]");

    let rv = render!(
        "{% for item in iter %}- {{ item }}: {{ loop.index }} / {{ loop.length }}\n{% endfor %}",
        iter => Value::make_iterable(|| 'a'..'f')
    );
    assert_snapshot!(rv, @r###"
    - a: 1 / 5
    - b: 2 / 5
    - c: 3 / 5
    - d: 4 / 5
    - e: 5 / 5
    "###);

    let rv = render!(
        "{% for item in iter %}- {{ item }}: {{ loop.index }} / {{ loop.length|default('?') }}\n{% endfor %}",
     iter => Value::make_iterable(|| (0..10).filter(|x| x % 2 == 0))
    );
    assert_snapshot!(rv, @r###"
    - 0: 1 / ?
    - 2: 2 / ?
    - 4: 3 / ?
    - 6: 4 / ?
    - 8: 5 / ?
    "###);
}

#[test]
fn test_complex_key() {
    let value = Value::from_iter([
        (Value::from_iter([0u32, 0u32]), "origin"),
        (Value::from_iter([0u32, 1u32]), "right"),
    ]);
    assert_eq!(
        value.get_item(&Value::from_iter([0, 0])).ok(),
        Some(Value::from("origin"))
    );
    assert_eq!(
        value.get_item(&Value::from_iter([0, 42])).ok(),
        Some(Value::UNDEFINED)
    );
}

#[test]
#[cfg(feature = "deserialization")]
fn test_deserialize() {
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    struct Point {
        x: i32,
        y: i32,
    }

    let point_value = Value::from_iter([("x", Value::from(42)), ("y", Value::from(-23))]);
    let point = Point::deserialize(point_value).unwrap();

    assert_eq!(point, Point { x: 42, y: -23 });
}

#[test]
#[cfg(feature = "deserialization")]
fn test_via_deserialize() {
    use minijinja::value::ViaDeserialize;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    struct Point {
        x: i32,
        y: i32,
    }

    fn foo(point: ViaDeserialize<Point>) -> String {
        format!("{}, {}", point.x, point.y)
    }

    let point_value = Value::from_iter([("x", Value::from(42)), ("y", Value::from(-23))]);

    let mut env = Environment::new();
    env.add_filter("foo", foo);
    let state = env.empty_state();

    let rv = state.apply_filter("foo", args![point_value]).unwrap();
    assert_eq!(rv.to_string(), "42, -23");
}

#[test]
fn test_seq_custom_iter() {
    #[derive(Debug)]
    struct WeirdSeq;

    impl Object for WeirdSeq {
        fn repr(self: &Arc<Self>) -> ObjectRepr {
            ObjectRepr::Seq
        }

        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Iter(Box::new(('a'..='b').map(Value::from)))
        }

        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_usize() {
                Some(0) => Some(Value::from(true)),
                Some(1) => Some(Value::from(false)),
                _ => None,
            }
        }
    }

    let v = Value::from_object(WeirdSeq);
    assert_eq!(v.get_item_by_index(0).unwrap(), Value::from(true));
    assert_eq!(v.get_item_by_index(1).unwrap(), Value::from(false));

    let vec = v.try_iter().unwrap().collect::<Vec<_>>();
    assert_eq!(vec, vec![Value::from('a'), Value::from('b')]);

    let obj = v.as_object().unwrap();
    let vec = obj.try_iter_pairs().unwrap().collect::<Vec<_>>();
    assert_eq!(
        vec,
        vec![
            (Value::from(0), Value::from('a')),
            (Value::from(1), Value::from('b'))
        ]
    );
}

#[test]
fn test_map_custom_iter() {
    #[derive(Debug)]
    struct WeirdMap;

    impl Object for WeirdMap {
        fn repr(self: &Arc<Self>) -> ObjectRepr {
            ObjectRepr::Map
        }

        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Iter(Box::new(('a'..='b').map(Value::from)))
        }

        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str() {
                Some("a") => Some(Value::from(true)),
                Some("b") => Some(Value::from(false)),
                _ => None,
            }
        }
    }

    let v = Value::from_object(WeirdMap);
    assert_eq!(v.get_attr("a").unwrap(), Value::from(true));
    assert_eq!(v.get_attr("b").unwrap(), Value::from(false));

    let vec = v.try_iter().unwrap().collect::<Vec<_>>();
    assert_eq!(vec, vec![Value::from('a'), Value::from('b')]);

    let obj = v.as_object().unwrap();
    let vec = obj.try_iter_pairs().unwrap().collect::<Vec<_>>();
    assert_eq!(
        vec,
        vec![
            (Value::from("a"), Value::from(true)),
            (Value::from("b"), Value::from(false))
        ]
    );
}
