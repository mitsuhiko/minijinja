use minijinja::value::{Tuple, Value, ValueKind};
use minijinja::{context, Environment};

#[test]
fn test_tuple_literals() {
    let env = Environment::new();

    assert_eq!(env.render_str("{{ () }}", ()).unwrap(), "()");
    assert_eq!(env.render_str("{{ (1,) }}", ()).unwrap(), "(1,)");
    assert_eq!(env.render_str("{{ (1, 2) }}", ()).unwrap(), "(1, 2)");
    assert_eq!(
        env.render_str("{{ (value, value + 1) }}", context!(value => 1))
            .unwrap(),
        "(1, 2)"
    );
    assert_eq!(
        env.render_str("{% set value = 1, 2 %}{{ value }}", ())
            .unwrap(),
        "(1, 2)"
    );
}

#[test]
fn test_python_compatible_collection_rendering() {
    let env = Environment::new();
    let source = r#"{{ [none, true, false, "plain", "has\u0027quote", "has\"quote", "both\u0027\"quotes", "\x13"] }}"#;
    assert_eq!(
        env.render_str(source, ()).unwrap(),
        r#"[None, True, False, 'plain', "has'quote", 'has"quote', 'both\'"quotes', '\x13']"#
    );
    assert_eq!(
        env.render_str(r#"{{ ("bar", "baz") }}"#, ()).unwrap(),
        "('bar', 'baz')"
    );
    assert_eq!(
        env.render_str(r#"{{ {"key": "value"} }}"#, ()).unwrap(),
        "{'key': 'value'}"
    );
}

#[test]
fn test_tuple_sequence_behavior() {
    let env = Environment::new();
    assert_eq!(
        env.render_str(
            "{{ value.0 }}|{{ value[-1] }}|{% for item in value %}{{ item }}{% endfor %}",
            context!(value => (1, 2)),
        )
        .unwrap(),
        "1|2|12"
    );
    assert_eq!(
        env.render_str("{{ (1, 2) == [1, 2] }}", ()).unwrap(),
        "False"
    );
    assert_eq!(env.render_str("{{ (1, 2)|list }}", ()).unwrap(), "[1, 2]");
}

#[test]
fn test_tuple_operations_preserve_type() {
    let env = Environment::new();
    assert_eq!(
        env.render_str("{{ (1, 2) + (3,) }}", ()).unwrap(),
        "(1, 2, 3)"
    );
    assert_eq!(
        env.render_str("{{ (1, 2) * 2 }}", ()).unwrap(),
        "(1, 2, 1, 2)"
    );
    assert_eq!(env.render_str("{{ (1, 2, 3)[1:] }}", ()).unwrap(), "(2, 3)");
    assert_eq!(
        env.render_str("{{ (1, 2, 3)[::-1] }}", ()).unwrap(),
        "(3, 2, 1)"
    );
    assert!(env.render_str("{{ [1, 2] + (3,) }}", ()).is_err());
}

#[test]
fn test_rust_and_serde_tuples() {
    let explicit = Value::from(Tuple::from([Value::from(1), Value::from(2)]));
    assert!(explicit.is_tuple());
    assert_eq!(explicit.kind(), ValueKind::Seq);
    assert_eq!(explicit.to_string(), "(1, 2)");
    assert_ne!(explicit, Value::from(vec![1, 2]));

    let converted = Value::from((1, 2));
    assert!(converted.is_tuple());
    assert_eq!(converted, explicit);

    #[cfg(feature = "serde")]
    {
        use minijinja::value::Serde;

        let serialized = Value::from(Serde((1, 2)));
        assert!(serialized.is_tuple());
        assert_eq!(serialized, explicit);
        assert_eq!(Value::from_serialize((1, 2)), serialized);
    }
}

#[test]
fn test_collecting_tuples_and_pairs() {
    let sequence: Value = [("key", 42)].into_iter().collect();
    assert_eq!(sequence.to_string(), "[('key', 42)]");

    let map = Value::from_pairs([("key", 42)]);
    assert_eq!(map.to_string(), "{'key': 42}");
}
