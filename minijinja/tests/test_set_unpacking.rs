//! Minimal regression for tuple unpacking in set statements.

use minijinja::Environment;

#[test]
fn test_set_single_var() {
    let env = Environment::new();
    let result = env.render_str("{%- set x = 42 -%}x={{ x }}", ()).unwrap();
    assert_eq!(result, "x=42");
}

#[test]
fn test_set_tuple_parens() {
    let env = Environment::new();
    let result = env
        .render_str("{%- set (a, b) = (1, 2) -%}a={{ a }}, b={{ b }}", ())
        .unwrap();
    assert_eq!(result, "a=1, b=2");
}

#[test]
fn test_set_unpacked_no_parens() {
    let env = Environment::new();
    let result = env
        .render_str(
            r#"{%- set a, b, c, d = '', '', true, "default" -%}values: a={{ a }}, b={{ b }}, c={{ c }}, d={{ d }}"#,
            (),
        )
        .unwrap();
    assert_eq!(
        result,
        "values: a=, b=, c=true, d=default"
    );
}
