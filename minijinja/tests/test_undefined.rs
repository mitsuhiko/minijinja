#![cfg(feature = "builtins")]
use std::collections::HashMap;

use minijinja::{context, render, Environment, ErrorKind, State, UndefinedBehavior, Value};

use similar_asserts::assert_eq;

fn empty_map() -> Value {
    Value::from_pairs(HashMap::<String, String>::new())
}

#[test]
fn test_lenient_undefined() {
    let mut env = Environment::new();
    env.add_filter("test", |state: &State, value: String| -> String {
        assert_eq!(state.undefined_behavior(), UndefinedBehavior::Lenient);
        assert_eq!(value, "");
        value
    });

    assert_eq!(env.undefined_behavior(), UndefinedBehavior::Lenient);
    assert_eq!(render!(in env, "<{{ true.missing_attribute }}>"), "<>");
    assert_eq!(
        env.render_str("{{ undefined.missing_attribute }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        render!(in env, "<{% for x in undefined %}...{% endfor %}>"),
        "<>"
    );
    assert_eq!(render!(in env, "{{ 'foo' is in(undefined) }}"), "False");
    assert_eq!(render!(in env, "<{{ undefined }}>"), "<>");
    assert_eq!(render!(in env, "{{ not undefined }}"), "True");
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "True");
    assert_eq!(
        render!(in env, "{{ x.foo is undefined }}", x => empty_map()),
        "True"
    );
    assert_eq!(render!(in env, "{{ undefined|list }}"), "[]");
    assert_eq!(render!(in env, "<{{ undefined|test }}>"), "<>");
    assert_eq!(render!(in env, "{{ 42 in undefined }}"), "False");
}

#[test]
fn test_semi_strict_undefined() {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::SemiStrict);
    env.add_filter(
        "test_rest_join",
        |_state: &State, values: minijinja::value::Rest<String>| -> String { values.join("|") },
    );
    env.add_filter(
        "test_vec_join",
        |_state: &State, values: Vec<String>| -> String { values.join("|") },
    );

    assert_eq!(
        env.render_str("{{ true.missing_attribute }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined.missing_attribute }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("<{% for x in undefined %}...{% endfor %}>", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ 'foo' is in(undefined) }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(render!(in env, "<{% if undefined %}42{% endif %}>"), "<>");
    assert_eq!(
        env.render_str("<{{ undefined }}>", ()).unwrap_err().kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(render!(in env, "{{ not undefined }}"), "True");
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "True");
    assert_eq!(render!(in env, "<{{ 42 if false }}>"), "<>");
    assert_eq!(
        render!(in env, "{{ x.foo is undefined }}", x => empty_map()),
        "True"
    );
    assert_eq!(
        env.render_str(
            "<{% if x.foo %}...{% endif %}>",
            context! { x => empty_map() }
        )
        .unwrap(),
        "<>"
    );
    assert_eq!(
        env.render_str("{{ undefined|list }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidOperation
    );
    assert_eq!(
        env.render_str("{{ 42 in undefined }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|upper }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|int }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|float }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|string }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|test_rest_join }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ [undefined]|test_vec_join }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    // bool follows is_true semantics: SemiStrict allows it (returns false), Strict errors
    assert_eq!(render!(in env, "{{ undefined|bool }}"), "False");
    // none|int should still work (undefined check only applies to undefined, not none)
    assert_eq!(render!(in env, "{{ none|int }}"), "0");
    assert_eq!(
        env.render_str("{{ undefined|default('FALLBACK') }}", ())
            .unwrap(),
        "FALLBACK"
    );
    assert_eq!(
        env.render_str("{{ foo == 'foo' }}", ()).unwrap_err().kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ foo ~ 'x' }}", ()).unwrap_err().kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ foo|default(bar) }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
}

#[test]
fn test_strict_undefined() {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.add_filter(
        "test_rest_join",
        |_state: &State, values: minijinja::value::Rest<String>| -> String { values.join("|") },
    );
    env.add_filter(
        "test_vec_join",
        |_state: &State, values: Vec<String>| -> String { values.join("|") },
    );

    assert_eq!(
        env.render_str("{{ true.missing_attribute }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined.missing_attribute }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("<{% for x in undefined %}...{% endfor %}>", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ 'foo' is in(undefined) }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("<{% if undefined %}42{% endif %}>", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("<{{ undefined }}>", ()).unwrap_err().kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("<{{ not undefined }}>", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "True");
    assert_eq!(env.render_str("<{{ 42 if false }}>", ()).unwrap(), "<>");
    assert_eq!(
        render!(in env, "{{ x.foo is undefined }}", x => empty_map()),
        "True"
    );
    assert_eq!(
        env.render_str(
            "{% if x.foo %}...{% endif %}",
            context! { x => empty_map() }
        )
        .unwrap_err()
        .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|list }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidOperation
    );
    assert_eq!(
        env.render_str("{{ 42 in undefined }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined in [1, 2, 3] }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined in 'abc' }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|upper }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|int }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|float }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|bool }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|string }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ undefined|test_rest_join }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(
        env.render_str("{{ [undefined]|test_vec_join }}", ())
            .unwrap_err()
            .kind(),
        ErrorKind::UndefinedError
    );
    // none|int should still work (only undefined is rejected)
    assert_eq!(render!(in env, "{{ none|int }}"), "0");
    assert_eq!(
        env.render_str("{{ undefined|default('FALLBACK') }}", ())
            .unwrap(),
        "FALLBACK"
    );
}

#[test]
fn test_chainable_undefined() {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Chainable);
    env.add_filter("test", |state: &State, value: String| -> String {
        assert_eq!(state.undefined_behavior(), UndefinedBehavior::Chainable);
        assert_eq!(value, "");
        value
    });

    assert_eq!(render!(in env, "<{{ true.missing_attribute }}>"), "<>");
    assert_eq!(render!(in env, "<{{ undefined.missing_attribute }}>"), "<>");
    assert_eq!(
        render!(in env, "<{% for x in undefined %}...{% endfor %}>"),
        "<>"
    );
    assert_eq!(
        render!(in env, "{{ x.foo is undefined }}", x => empty_map()),
        "True"
    );
    assert_eq!(render!(in env, "{{ 'foo' is in(undefined) }}"), "False");
    assert_eq!(render!(in env, "<{{ undefined }}>"), "<>");
    assert_eq!(render!(in env, "{{ not undefined }}"), "True");
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "True");
    assert_eq!(render!(in env, "{{ undefined|list }}"), "[]");
    assert_eq!(render!(in env, "<{{ undefined|test }}>"), "<>");
    assert_eq!(render!(in env, "{{ 42 in undefined }}"), "False");
}

#[test]
fn test_undefined_sequence_filters() {
    // Like in Jinja2, undefined values behave like empty sequences in these
    // filters unless strict undefined behavior is used.
    let filters = [
        ("undefined|first is undefined", "True"),
        ("undefined|last is undefined", "True"),
        ("undefined|reverse|list", "[]"),
        ("undefined|length", "0"),
        ("undefined|count", "0"),
        ("undefined|items|list", "[]"),
        ("undefined|join(',')", ""),
        ("undefined|groupby('x')|list", "[]"),
    ];

    for behavior in [UndefinedBehavior::Lenient, UndefinedBehavior::Chainable] {
        let mut env = Environment::new();
        env.set_undefined_behavior(behavior);
        for (expr, expected) in filters {
            let source = format!("{{{{ {expr} }}}}");
            assert_eq!(
                env.render_str(&source, ()).unwrap(),
                expected,
                "{expr} with {behavior:?}"
            );
        }
    }

    for behavior in [UndefinedBehavior::SemiStrict, UndefinedBehavior::Strict] {
        let mut env = Environment::new();
        env.set_undefined_behavior(behavior);
        for (expr, _) in filters {
            let source = format!("{{{{ {expr} }}}}");
            let err = env.render_str(&source, ()).unwrap_err();
            assert_eq!(
                err.kind(),
                ErrorKind::UndefinedError,
                "{expr} with {behavior:?}"
            );
            // silent undefined values are still accepted
            let source = format!(
                "{{% set silent = 42 if false %}}{{{{ {} }}}}",
                expr.replacen("undefined", "silent", 1)
            );
            assert!(
                env.render_str(&source, ()).is_ok(),
                "{expr} with silent undefined and {behavior:?}"
            );
        }
    }
}

#[test]
fn test_reverse_none() {
    // Like in Jinja2 none cannot be reversed, even though it iterates as
    // an empty sequence elsewhere.
    let env = Environment::new();
    let err = env.render_str("{{ none|reverse }}", ()).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert_eq!(err.detail(), Some("cannot reverse values of type none"));
    assert_eq!(render!(in env, "{{ none|list }}"), "[]");
    // the value API keeps returning none unchanged
    assert!(Value::from(()).reverse().unwrap().is_none());
}
