#![cfg(feature = "pycompat")]
use minijinja::{Environment, Value};
use minijinja_contrib::pycompat::unknown_method_callback;
use similar_asserts::assert_eq;

fn eval_expr(expr: &str) -> Value {
    let mut env = Environment::new();
    env.set_unknown_method_callback(unknown_method_callback);
    env.compile_expression(expr).unwrap().eval(()).unwrap()
}

#[test]
fn test_string_methods() {
    assert_eq!(eval_expr("'foo'.upper()").as_str(), Some("FOO"));
    assert_eq!(eval_expr("'FoO'.lower()").as_str(), Some("foo"));
    assert_eq!(eval_expr("' foo '.strip()").as_str(), Some("foo"));
    assert_eq!(eval_expr("'!foo?!!!'.strip('!?')").as_str(), Some("foo"));
    assert_eq!(
        eval_expr("'!!!foo?!!!'.rstrip('!?')").as_str(),
        Some("!!!foo")
    );
    assert_eq!(
        eval_expr("'!!!foo?!!!'.lstrip('!?')").as_str(),
        Some("foo?!!!")
    );
    assert!(eval_expr("'foobar'.islower()").is_true());
    assert!(eval_expr("'FOOBAR'.isupper()").is_true());
    assert!(eval_expr("' \\n'.isspace()").is_true());
    assert_eq!(
        eval_expr("'foobar'.replace('o', 'x')").as_str(),
        Some("fxxbar")
    );
    assert_eq!(
        eval_expr("'foobar'.replace('o', 'x', 1)").as_str(),
        Some("fxobar")
    );
    assert_eq!(eval_expr("'foo bar'.title()").as_str(), Some("Foo Bar"));
    assert_eq!(
        eval_expr("'foo bar'.capitalize()").as_str(),
        Some("Foo bar")
    );
    assert_eq!(eval_expr("'foo barooo'.count('oo')").as_usize(), Some(2));
    assert_eq!(eval_expr("'foo barooo'.find('oo')").as_usize(), Some(1));
}

#[test]
fn test_dict_methods() {
    assert!(eval_expr("{'x': 42}.keys()|list == ['x']").is_true());
    assert!(eval_expr("{'x': 42}.values()|list == [42]").is_true());
    assert!(eval_expr("{'x': 42}.items()|list == [('x', 42)]").is_true());
    assert!(eval_expr("{'x': 42}.get('x') == 42").is_true());
    assert!(eval_expr("{'x': 42}.get('y') is none").is_true());
}

#[test]
fn test_list_methods() {
    assert!(eval_expr("[1, 2, 2, 3].count(2) == 2").is_true());
}
