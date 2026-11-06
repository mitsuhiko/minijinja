#![cfg(feature = "builtins")]

use minijinja::{context, Environment};

#[test]
fn test_constant_logical_operators_preserve_operands() {
    let env = Environment::new();
    assert_eq!(env.render_str("{{ 7 and 0 }}", ()).unwrap(), "0");
    assert_eq!(env.render_str("{{ 0 and 7 }}", ()).unwrap(), "0");
    assert_eq!(env.render_str("{{ 0 or 7 }}", ()).unwrap(), "7");
    assert_eq!(env.render_str("{{ 7 or 0 }}", ()).unwrap(), "7");
}

#[test]
fn test_repeated_sequence_size_limit() {
    let err = Environment::new()
        .render_str("{{ [0] * 2**30 }}", ())
        .unwrap_err();
    assert!(err.to_string().contains("repeated sequence is too large"));
}

#[test]
fn test_upper_and_lower_reject_non_strings() {
    let env = Environment::new();
    assert_eq!(
        env.render_str(
            "{{ missing is upper }}|{{ 1 is upper }}|{{ missing is lower }}|{{ 1 is lower }}",
            (),
        )
        .unwrap(),
        "False|False|False|False"
    );
    assert_eq!(
        env.render_str(
            "{{ 'FOO 1' is upper }}|{{ 'foo 1' is lower }}|{{ '' is upper }}|{{ 'Aǅ' is upper }}|{{ 'aǅ' is lower }}",
            ()
        )
        .unwrap(),
        "True|True|False|False|False"
    );
}

#[test]
fn test_undefined_is_a_sequence() {
    let env = Environment::new();
    assert_eq!(
        env.render_str("{{ missing is iterable }}|{{ missing is sequence }}", ())
            .unwrap(),
        "True|True"
    );
}

#[test]
fn test_booleans_are_numbers() {
    let env = Environment::new();
    assert_eq!(
        env.render_str(
            "{{ true is number }}|{{ [true, 1]|select('number')|list }}|{{ [true, true]|sum }}",
            (),
        )
        .unwrap(),
        "True|[True, 1]|2"
    );
}

#[test]
fn test_division_by_zero_errors() {
    let env = Environment::new();
    for source in ["{{ 1 / 0 }}", "{{ -1 / 0 }}", "{{ 0 / 0 }}"] {
        assert!(env.render_str(source, ()).is_err(), "{source}");
    }
}

#[test]
fn test_round_uses_bankers_rounding() {
    let env = Environment::new();
    assert_eq!(
        env.render_str("{{ 2.5|round }}|{{ 2.675|round(2) }}|{{ -2.5|round }}", ())
            .unwrap(),
        "2.0|2.67|-2.0"
    );
    assert_eq!(
        env.render_str(
            "{{ 250.0|round(-2) }}|{{ 350.0|round(-2) }}|{{ 9.5e22|round(-22) }}",
            (),
        )
        .unwrap(),
        "200.0|400.0|90000000000000000000000.0"
    );
    assert_eq!(
        env.render_str(
            "{{ positive|round(-309) }}|{{ negative|round(-309) }}",
            context!(positive => f64::INFINITY, negative => f64::NEG_INFINITY),
        )
        .unwrap(),
        "inf|-inf"
    );
    assert!(env
        .render_str("{{ value|round(-308) }}", context!(value => 1.7e308))
        .is_err());
}

#[cfg(feature = "multi_template")]
#[test]
fn test_super_in_included_template() {
    let mut env = Environment::new();
    env.add_template("p", "{% block a %}P{% endblock %}")
        .unwrap();
    env.add_template("i", "[{{ super() }}]").unwrap();
    env.add_template(
        "c",
        "{% extends 'p' %}{% block a %}C{% include 'i' %}{% endblock %}",
    )
    .unwrap();
    let err = env.get_template("c").unwrap().render(()).unwrap_err();
    let source = std::error::Error::source(&err).unwrap().to_string();
    assert!(source.contains("cannot super outside of block"), "{source}");
}

#[cfg(feature = "multi_template")]
#[test]
fn test_loop_recursion_outside_of_loop_instructions() {
    let mut env = Environment::new();
    env.add_template("i", "{{ loop(x[1:] if x is sequence else []) }}")
        .unwrap();
    // included template
    env.add_template(
        "include",
        "{% for x in [[1,[2]]] recursive %}<{{ x }}{% include 'i' %}>{% endfor %}",
    )
    .unwrap();
    // block inside of a loop
    env.add_template(
        "block",
        "{% for x in [[1,[2]]] recursive %}<{{ x }}\
         {% block b %}{{ loop(x[1:] if x is sequence else []) }}{% endblock %}>{% endfor %}",
    )
    .unwrap();
    // loop passed to a macro
    env.add_template(
        "macro",
        "{% macro m(l, x) %}{{ l(x[1:] if x is sequence else []) }}{% endmacro %}\
         {% for x in [[1,[2]]] recursive %}<{{ x }}{{ m(loop, x) }}>{% endfor %}",
    )
    .unwrap();
    for name in ["include", "block", "macro"] {
        let err = env.get_template(name).unwrap().render(()).unwrap_err();
        let mut msg = err.to_string();
        if let Some(source) = std::error::Error::source(&err) {
            msg.push_str(&source.to_string());
        }
        assert!(
            msg.contains("cannot recurse outside of recursive loop"),
            "{name}: {msg}"
        );
    }

    // recursing an outer loop from within the same template still works
    env.add_template(
        "outer",
        "{% for x in [[1,[2]]] recursive %}{% set outer = loop %}<{{ x }}\
         {% for y in [1] %}{{ outer(x[1:] if x is sequence else []) }}{% endfor %}>{% endfor %}",
    )
    .unwrap();
    assert_eq!(
        env.get_template("outer").unwrap().render(()).unwrap(),
        "<[1, [2]]<[2]>>"
    );
}
