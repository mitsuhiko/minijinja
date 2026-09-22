#![cfg(feature = "builtins")]

use minijinja::Environment;

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
            "{{ 'FOO 1' is upper }}|{{ 'foo 1' is lower }}|{{ '' is upper }}",
            ()
        )
        .unwrap(),
        "True|True|False"
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
        env.render_str("{{ 250.0|round(-2) }}|{{ 350.0|round(-2) }}", ())
            .unwrap(),
        "200.0|400.0"
    );
}
