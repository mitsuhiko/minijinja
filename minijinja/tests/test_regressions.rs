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
