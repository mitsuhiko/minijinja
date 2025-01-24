#![cfg(feature = "builtins")]
use std::collections::HashMap;

use minijinja::{context, render, Environment, ErrorKind, State, UndefinedBehavior, Value};

use similar_asserts::assert_eq;

macro_rules! assert_undefined_error {
    ($env:expr, $template:expr) => {
        assert_undefined_error!($env, $template, ())
    };
    ($env:expr, $template:expr, $context:expr) => {
        assert_eq!(
            $env.render_str($template, $context).unwrap_err().kind(),
            ErrorKind::UndefinedError
        );
    };
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
    assert_eq!(render!(in env, "{{ 'foo' is in(undefined) }}"), "false");
    assert_eq!(render!(in env, "<{{ undefined }}>"), "<>");
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "true");
    assert_eq!(
        render!(in env, "{{ x.foo is undefined }}", x => HashMap::<String, String>::new()),
        "true"
    );
    assert_eq!(render!(in env, "{{ undefined|list }}"), "[]");
    assert_eq!(render!(in env, "<{{ undefined|test }}>"), "<>");
    assert_eq!(render!(in env, "{{ 42 in undefined }}"), "false");
}

#[test]
fn test_strict_undefined() {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);

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
        env.render_str("<{{ undefined }}>", ()).unwrap_err().kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "true");
    assert_eq!(
        render!(in env, "{{ x.foo is undefined }}", x => HashMap::<String, String>::new()),
        "true"
    );
    assert_eq!(
        env.render_str(
            "{% if x.foo %}...{% endif %}",
            context! { x => HashMap::<String, String>::new() }
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

    assert_eq!(render!(in env, "{{ undefined | default(42) }}"), "42");
    assert_eq!(render!(in env, "{{ 'foobar' if not undefined }}"), "foobar");

    let context = context! { undefvar => Value::UNDEFINED };
    assert_eq!(
        render!(in env, "{{ undefvar | default(42)  }}", context),
        "42"
    );

    assert_undefined_error!(env, "{{ 'foobar' if undefvar else 'baz'  }}", &context);

    // TODO is this inconsistent with the test above?
    assert_eq!(
        render!(in env, "{{ 'foobar' if not undefvar }}", context),
        "foobar"
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
        render!(in env, "{{ x.foo is undefined }}", x => HashMap::<String, String>::new()),
        "true"
    );
    assert_eq!(render!(in env, "{{ 'foo' is in(undefined) }}"), "false");
    assert_eq!(render!(in env, "<{{ undefined }}>"), "<>");
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "true");
    assert_eq!(render!(in env, "{{ undefined|list }}"), "[]");
    assert_eq!(render!(in env, "<{{ undefined|test }}>"), "<>");
    assert_eq!(render!(in env, "{{ 42 in undefined }}"), "false");
}

#[test]
fn test_medium_strict_undefined() {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::MediumStrict);

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
        env.render_str("<{{ undefined }}>", ()).unwrap_err().kind(),
        ErrorKind::UndefinedError
    );
    assert_eq!(render!(in env, "{{ undefined is undefined }}"), "true");
    assert_eq!(
        render!(in env, "{{ x.foo is undefined }}", x => HashMap::<String, String>::new()),
        "true"
    );
    // TODO figure out why this passes
    // assert_eq!(
    //     env.render_str(
    //         "{% if x.foo %}...{% endif %}",
    //         context! { x => HashMap::<String, String>::new() }
    //     )
    //     .unwrap_err()
    //     .kind(),
    //     ErrorKind::UndefinedError
    // );
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

    assert_eq!(render!(in env, "{{ undefined | default(42) }}"), "42");
    assert_eq!(render!(in env, "{{ 'foobar' if not undefined }}"), "foobar");

    let context = context! { undefvar => Value::UNDEFINED };
    assert_eq!(
        render!(in env, "{{ undefvar | default(42)  }}", context),
        "42"
    );

    // This is the main change with respect to the strict behavior
    assert_eq!(
        render!(in env, "{{ 'foobar' if undefvar else 'baz'  }}", context),
        "baz"
    );
    assert_eq!(
        render!(in env, "{{ 'foobar' if not undefvar  }}", context),
        "foobar"
    );
}
