use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};
use std::sync::Arc;

use insta::{assert_debug_snapshot, assert_snapshot};
use similar_asserts::assert_eq;

use minijinja::value::{DynObject, Enumerator, Kwargs, Object, ObjectRepr, Rest, Value};
use minijinja::{args, context, render, Environment, Error, ErrorKind};

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
        false,
        true,
        -inf,
        -100,
        -75.0,
        -50.0,
        0,
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
    let v2 = Value::from_serialize(&v);
    assert!(v.is_safe());
    assert!(v2.is_safe());
    assert_eq!(v.to_string(), v2.to_string());
}

#[test]
fn test_undefined_roundtrip() {
    let v = Value::UNDEFINED;
    let v2 = Value::from_serialize(&v);
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
    assert_eq!(obj.enumerator_len(), Some(4));
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
    use minijinja::value::{from_args, ViaDeserialize};
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    struct Point {
        x: i32,
        y: i32,
    }

    let point_value = Value::from_iter([("x", Value::from(42)), ("y", Value::from(-23))]);
    let point = Point::deserialize(point_value).unwrap();

    assert_eq!(point, Point { x: 42, y: -23 });

    #[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
    enum SimpleEnum {
        B,
        C,
        D,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
    enum TaggedUnion {
        V(String),
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
    struct UnitStruct(String);

    let spe = Value::from_serialize(SimpleEnum::B);
    let spu = Value::from_serialize(UnitStruct("hello".into()));
    let spt = Value::from_serialize(TaggedUnion::V("workd".into()));

    let a: (
        ViaDeserialize<SimpleEnum>,
        ViaDeserialize<UnitStruct>,
        ViaDeserialize<TaggedUnion>,
    ) = from_args(args!(spe, spu, spt)).unwrap();
    assert_eq!((a.0).0, SimpleEnum::B);
    assert_eq!((a.1).0, UnitStruct("hello".into()));
    assert_eq!((a.2).0, TaggedUnion::V("workd".into()));
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

#[test]
fn test_plain_object() {
    #[derive(Debug)]
    struct X;

    impl Object for X {
        fn repr(self: &Arc<Self>) -> ObjectRepr {
            ObjectRepr::Plain
        }
    }

    let x = Value::from_object(X);
    assert!(x.try_iter().is_err());
    assert_snapshot!(render!("{{ x }}|{{ x.missing_attr is undefined }}", x), @"X|true");
}

#[test]
fn test_reverse() {
    // reverse vectors
    assert_snapshot!(Value::from_iter(0..3).reverse().unwrap(), @"[2, 1, 0]");
    // regular iterators (non reversible)
    assert_snapshot!(Value::make_iterable(|| 0..3).reverse().unwrap(), @"[2, 1, 0]");
    // strings
    assert_snapshot!(Value::from("abc").reverse().unwrap(), @"cba");
    // bytes
    assert_snapshot!(Value::from_serialize(b"abc").reverse().unwrap(), @"[99, 98, 97]");
    // undefined
    assert!(Value::UNDEFINED.reverse().unwrap().is_undefined());
    // none
    assert!(Value::from(()).reverse().unwrap().is_none());

    #[derive(Debug)]
    struct OddMap;

    impl Object for OddMap {
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            Some(key.clone())
        }
        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Str(&["a", "b", "c"])
        }
    }

    let odd_map = Value::from_object(OddMap);
    assert_eq!(odd_map.len(), Some(3));
    assert_snapshot!(
        render!("{% set r = m|reverse %}{{ m }}|{{ r }}|{{ r }}", m => odd_map),
        @r###"{"a": "a", "b": "b", "c": "c"}|["c", "b", "a"]|["c", "b", "a"]"###
    );

    #[derive(Debug)]
    struct RevThing;

    impl Object for RevThing {
        fn repr(self: &Arc<Self>) -> ObjectRepr {
            ObjectRepr::Iterable
        }
        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::RevIter(Box::new((0..3).map(Value::from)))
        }
    }

    let rev_thing = Value::from_object(RevThing);
    assert_eq!(rev_thing.len(), Some(3));
    assert_snapshot!(
        render!("{% set r = m|reverse %}{{ m }}|{{ r }}|{{ r }}", m => rev_thing),
        @"[0, 1, 2]|[0, 1, 2]|[0, 1, 2]"
    );

    #[derive(Debug)]
    struct ValueThing;

    impl Object for ValueThing {
        fn repr(self: &Arc<Self>) -> ObjectRepr {
            ObjectRepr::Iterable
        }
        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Values(vec![false.into(), true.into()])
        }
    }

    let value_thing = Value::from_object(ValueThing);
    assert_eq!(value_thing.len(), Some(2));
    assert_snapshot!(
        render!("{% set r = m|reverse %}{{ m }}|{{ r }}|{{ r }}", m => value_thing),
        @"[false, true]|[true, false]|[true, false]"
    );

    assert_snapshot!(
        Value::from(42).reverse().unwrap_err(),
        @"invalid operation: cannot reverse values of type number"
    );
}

#[test]
fn test_object_vec() {
    let value = Value::from(vec![1i32, 2, 3, 4]);
    assert_eq!(
        value.downcast_object_ref::<Vec<i32>>(),
        Some(&vec![1, 2, 3, 4])
    );
    assert_eq!(
        value.get_item_by_index(0).ok().and_then(|x| x.as_i64()),
        Some(1)
    );
    let iter = value.try_iter().unwrap();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(
        iter.map(|x| x.as_i64().unwrap()).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(value.to_string(), "[1, 2, 3, 4]");
}

#[test]
#[cfg(feature = "std_collections")]
fn test_object_vec_deque() {
    let value = Value::from(VecDeque::from([1i32, 2, 3, 4]));
    assert_eq!(
        value.downcast_object_ref::<VecDeque<i32>>(),
        Some(&VecDeque::from([1, 2, 3, 4]))
    );
    assert_eq!(
        value.get_item_by_index(0).ok().and_then(|x| x.as_i64()),
        Some(1)
    );
    let iter = value.try_iter().unwrap();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(
        iter.map(|x| x.as_i64().unwrap()).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(value.to_string(), "[1, 2, 3, 4]");
}

#[test]
#[cfg(feature = "std_collections")]
fn test_object_linked_list() {
    let value = Value::from(LinkedList::from([1i32, 2, 3, 4]));
    assert_eq!(
        value.downcast_object_ref::<LinkedList<i32>>(),
        Some(&LinkedList::from([1, 2, 3, 4]))
    );
    let iter = value.try_iter().unwrap();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(
        iter.map(|x| x.as_i64().unwrap()).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(value.to_string(), "[1, 2, 3, 4]");
}

#[test]
#[cfg(feature = "std_collections")]
fn test_object_hash_set() {
    let value = Value::from(HashSet::from([1i32, 2, 3, 4]));
    assert_eq!(
        value.downcast_object_ref::<HashSet<i32>>(),
        Some(&HashSet::from([1, 2, 3, 4]))
    );
    let iter = value.try_iter().unwrap();
    let mut items = iter.map(|x| x.as_i64().unwrap()).collect::<Vec<_>>();
    items.sort();
    assert_eq!(items, vec![1, 2, 3, 4]);
}

#[test]
#[cfg(feature = "std_collections")]
fn test_object_btree_set() {
    let value = Value::from(BTreeSet::from([1i32, 2, 3, 4]));
    assert_eq!(
        value.downcast_object_ref::<BTreeSet<i32>>(),
        Some(&BTreeSet::from([1, 2, 3, 4]))
    );
    let iter = value.try_iter().unwrap();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(
        iter.map(|x| x.as_i64().unwrap()).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(value.to_string(), "[1, 2, 3, 4]");
}

#[test]
#[cfg(feature = "std_collections")]
fn test_object_hash_map() {
    let value = Value::from(HashMap::from_iter([("foo", 1i32), ("bar", 2)]));
    assert_eq!(
        value.downcast_object_ref::<HashMap<Arc<str>, i32>>(),
        Some(&HashMap::from_iter([
            (Arc::from("foo".to_string()), 1),
            (Arc::from("bar".to_string()), 2),
        ]))
    );
    let iter = value.try_iter().unwrap();
    assert_eq!(iter.size_hint(), (2, Some(2)));

    let value = Value::from(HashMap::from_iter([
        ("foo".to_string(), 1i32),
        ("bar".to_string(), 2),
    ]));
    assert_eq!(value.get_attr("foo").ok().and_then(|x| x.as_i64()), Some(1));
    assert_eq!(
        value.downcast_object_ref::<HashMap<String, i32>>(),
        Some(&HashMap::from_iter([
            ("foo".to_string(), 1),
            ("bar".to_string(), 2),
        ]))
    );

    let value = Value::from(HashMap::from_iter([("foo", 1i32)]));
    assert_eq!(
        value
            .try_iter()
            .unwrap()
            .map(|x| x.as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec!["foo"]
    );
    assert_eq!(value.to_string(), "{\"foo\": 1}");

    let value = Value::from(HashMap::from_iter([(Value::from(true), 1i32)]));
    assert_eq!(
        value
            .get_item(&Value::from(true))
            .ok()
            .and_then(|x| x.as_i64()),
        Some(1)
    );
    assert_eq!(value.to_string(), "{true: 1}");
}

#[test]
fn test_object_btree_map() {
    let value = Value::from(BTreeMap::from_iter([("foo", 1i32), ("bar", 2)]));
    assert_eq!(
        value.downcast_object_ref::<BTreeMap<Arc<str>, i32>>(),
        Some(&BTreeMap::from_iter([
            (Arc::from("foo".to_string()), 1),
            (Arc::from("bar".to_string()), 2),
        ]))
    );
    let iter = value.try_iter().unwrap();
    assert_eq!(iter.size_hint(), (2, Some(2)));

    let value = Value::from(BTreeMap::from_iter([
        ("foo".to_string(), 1i32),
        ("bar".to_string(), 2),
    ]));
    assert_eq!(value.get_attr("foo").ok().and_then(|x| x.as_i64()), Some(1));
    assert_eq!(
        value.downcast_object_ref::<BTreeMap<String, i32>>(),
        Some(&BTreeMap::from_iter([
            ("foo".to_string(), 1),
            ("bar".to_string(), 2),
        ]))
    );

    let value = Value::from(BTreeMap::from_iter([("foo", 1i32)]));
    assert_eq!(
        value
            .try_iter()
            .unwrap()
            .map(|x| x.as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec!["foo"]
    );
    assert_eq!(value.to_string(), "{\"foo\": 1}");

    let value = Value::from(BTreeMap::from_iter([(Value::from(true), 1i32)]));
    assert_eq!(
        value
            .get_item(&Value::from(true))
            .ok()
            .and_then(|x| x.as_i64()),
        Some(1)
    );
    assert_eq!(value.to_string(), "{true: 1}");
}

#[test]
fn test_downcast_arg() {
    #[derive(Debug)]
    struct A;

    #[derive(Debug)]
    struct B;

    fn my_func(a: &A, b: Arc<B>) -> String {
        format!("{:?}|{:?}", a, b)
    }

    impl Object for A {}
    impl Object for B {}

    let mut env = Environment::new();
    env.add_function("my_func", my_func);

    assert_eq!(
        render!(in env, "{{ my_func(a, b) }}",
        a => Value::from_object(A),
        b => Value::from_object(B)),
        "A|B"
    );
}

#[test]
fn test_map_eq() {
    #[derive(Debug, Copy, Clone)]
    struct Thing {
        rev: bool,
    }

    impl Object for Thing {
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str()? {
                "a" => Some(Value::from(1)),
                "b" => Some(Value::from(2)),
                _ => None,
            }
        }

        fn enumerate(self: &Arc<Self>) -> Enumerator {
            if self.rev {
                Enumerator::Str(&["b", "a"])
            } else {
                Enumerator::Str(&["a", "b"])
            }
        }
    }

    let t1 = Value::from_object(Thing { rev: false });
    let t2 = Value::from_object(Thing { rev: true });

    assert_snapshot!(t1.to_string(), @r###"{"a": 1, "b": 2}"###);
    assert_snapshot!(t2.to_string(), @r###"{"b": 2, "a": 1}"###);
    assert_eq!(t1, t2);
}

#[test]
fn test_float_eq() {
    let a = Value::from(2i128.pow(53));
    let b = Value::from(2.0f64.powf(53.0));
    assert_eq!(a, b);
    let xa = Value::from(i64::MAX as i128);
    let xb = Value::from(i64::MAX as f64);
    assert_ne!(xa, xb);
}

#[test]
fn test_eq_regression() {
    // merged objects used to not have a length.  let's make sure that they have
    let vars = context! {};
    let new_vars = context! {..vars.clone()};
    assert_eq!(vars.len(), Some(0));
    assert_eq!(new_vars.len(), Some(0));
    assert_eq!(&vars, &new_vars);

    // we also want to make sure that objects with unknown lengths are properly checked.
    #[derive(Debug)]
    struct MadMap;

    impl Object for MadMap {
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str()? {
                "a" => Some(Value::from(1)),
                "b" => Some(Value::from(2)),
                _ => None,
            }
        }

        fn enumerate(self: &Arc<Self>) -> Enumerator {
            let mut idx = 0;
            Enumerator::Iter(Box::new(std::iter::from_fn(move || {
                let new_idx = {
                    idx += 1;
                    idx
                };
                match new_idx {
                    1 => Some(Value::from("a")),
                    2 => Some(Value::from("b")),
                    _ => None,
                }
            })))
        }
    }

    let normal_map = context! {
        a => 1,
        b => 2
    };
    let mad_map = Value::from_object(MadMap);
    assert_eq!(mad_map.len(), None);
    assert_eq!(mad_map, normal_map);
    assert_eq!(normal_map, mad_map);
    assert_ne!(
        mad_map,
        context! {
            a => 1,
            b => 2,
            c => 3,
        }
    );
    assert_ne!(
        mad_map,
        context! {
            a => 1,
        }
    );
}

#[test]
fn test_sorting() {
    let mut values = vec![
        Value::from(-f64::INFINITY),
        Value::from(1.0),
        Value::from(f64::NAN),
        Value::from(f64::INFINITY),
        Value::from(42.0),
        Value::from(41),
        Value::from(128),
        Value::from(-2),
        Value::from(-5.0),
        Value::from(32i32),
        Value::from(true),
        Value::from(false),
        Value::from(vec![1, 2, 3]),
        Value::from(vec![1, 2, 3, 4]),
        Value::from(vec![1]),
        Value::from("whatever"),
        Value::from("floats"),
        Value::from("the"),
        Value::from("boat"),
        Value::UNDEFINED,
        Value::from(()),
        Value::from(Error::new(ErrorKind::InvalidOperation, "shit hit the fan")),
    ];
    values.sort();
    assert_debug_snapshot!(&values, @r###"
    [
        undefined,
        none,
        false,
        true,
        -inf,
        -5.0,
        -2,
        1.0,
        32,
        41,
        42.0,
        128,
        inf,
        NaN,
        "boat",
        "floats",
        "the",
        "whatever",
        [
            1,
        ],
        [
            1,
            2,
            3,
        ],
        [
            1,
            2,
            3,
            4,
        ],
        <invalid value: invalid operation: shit hit the fan>,
    ]
    "###);
}
