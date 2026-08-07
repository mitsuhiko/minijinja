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
fn test_rust_and_serde_tuples() {
    let explicit = Value::from(Tuple::from([Value::from(1), Value::from(2)]));
    assert!(explicit.is_tuple());
    assert_eq!(explicit.kind(), ValueKind::Seq);
    assert_eq!(explicit.to_string(), "(1, 2)");
    assert_ne!(explicit, Value::from(vec![1, 2]));

    let serialized = Value::from_serialize((1, 2));
    assert!(serialized.is_tuple());
    assert_eq!(serialized, explicit);
}
