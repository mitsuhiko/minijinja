use minijinja::value::{MaybeSafeStr, Value};
use minijinja::{args, Environment};
use similar_asserts::assert_eq;

use minijinja::filters::indent;

#[test]
fn test_filter_with_non() {
    fn filter(value: Option<String>) -> String {
        format!("[{}]", value.unwrap_or_default())
    }

    let mut env = Environment::new();
    env.add_filter("filter", filter);
    let state = env.empty_state();

    let rv = state
        .apply_filter("filter", args!(Value::UNDEFINED))
        .unwrap();
    assert_eq!(rv, Value::from("[]"));

    let rv = state
        .apply_filter("filter", args!(Value::from(())))
        .unwrap();
    assert_eq!(rv, Value::from("[]"));

    let rv = state
        .apply_filter("filter", args!(Value::from("wat")))
        .unwrap();
    assert_eq!(rv, Value::from("[wat]"));
}

#[test]
fn test_indent_one_empty_line() {
    let teststring = String::from("\n");
    assert_eq!(
        indent(teststring.into(), 2, None, None).to_string(),
        String::from("")
    );
}

#[test]
fn test_indent_one_line() {
    let teststring = String::from("test\n");
    assert_eq!(
        indent(teststring.into(), 2, None, None).to_string(),
        String::from("test")
    );
}

#[test]
fn test_indent() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring.into(), 2, None, None).to_string(),
        String::from("test\n  test1\n\n  test2")
    );
}

#[test]
fn test_indent_with_indented_first_line() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring.into(), 2, Some(true), None).to_string(),
        String::from("  test\n  test1\n\n  test2")
    );
}

#[test]
fn test_indent_with_indented_blank_line() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring.into(), 2, None, Some(true)).to_string(),
        String::from("test\n  test1\n  \n  test2")
    );
}

#[test]
fn test_indent_with_all_indented() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring.into(), 2, Some(true), Some(true)).to_string(),
        String::from("  test\n  test1\n  \n  test2")
    );
}

#[test]
fn test_indent_escaping() {
    let x = MaybeSafeStr::new_safe("<strong>Foo</strong>\n<i>bar</i>");
    let v = indent(x, 2, None, None);
    assert!(v.is_safe());
    assert_eq!(
        v.to_string(),
        String::from("<strong>Foo</strong>\n  <i>bar</i>")
    )
}
