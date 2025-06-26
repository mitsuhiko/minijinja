#![cfg(feature = "builtins")]
use minijinja::value::Value;
use minijinja::{args, context, Environment};
use similar_asserts::assert_eq;

use minijinja::filters::{abs, indent};

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
    assert_eq!(indent(teststring, 2, None, None), String::from(""));
}

#[test]
fn test_indent_one_line() {
    let teststring = String::from("test\n");
    assert_eq!(indent(teststring, 2, None, None), String::from("test"));
}

#[test]
fn test_indent() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring, 2, None, None),
        String::from("test\n  test1\n\n  test2")
    );
}

#[test]
fn test_indent_with_indented_first_line() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring, 2, Some(true), None),
        String::from("  test\n  test1\n\n  test2")
    );
}

#[test]
fn test_indent_with_indented_blank_line() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring, 2, None, Some(true)),
        String::from("test\n  test1\n  \n  test2")
    );
}

#[test]
fn test_indent_with_all_indented() {
    let teststring = String::from("test\ntest1\n\ntest2\n");
    assert_eq!(
        indent(teststring, 2, Some(true), Some(true)),
        String::from("  test\n  test1\n  \n  test2")
    );
}

#[test]
fn test_abs_overflow() {
    let ok = abs(Value::from(i64::MIN)).unwrap();
    assert_eq!(ok, Value::from(-(i64::MIN as i128)));
    let err = abs(Value::from(i128::MIN)).unwrap_err();
    assert_eq!(err.to_string(), "invalid operation: overflow on abs");
}

#[test]
fn test_chain_lists() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2] | chain([3, 4]) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[1, 2, 3, 4]");
}

#[test]
fn test_chain_length() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2] | chain([3, 4, 5]) | length }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "5");
}

#[test]
fn test_chain_dicts() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ {'a': 1} | chain({'b': 2}) | items | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, r#"[["a", 1], ["b", 2]]"#);
}

#[test]
fn test_chain_dict_lookup() {
    let env = Environment::new();
    // Last dict wins for lookups
    let tmpl = env
        .template_from_str("{{ ({'a': 1} | chain({'a': 2}))['a'] }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "2");
}

#[test]
fn test_chain_multiple() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1] | chain([2], [3, 4]) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[1, 2, 3, 4]");
}

#[test]
fn test_chain_with_iteration() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{% for item in [1, 2] | chain([3, 4]) %}{{ item }}{% endfor %}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "1234");
}

#[test]
fn test_chain_indexing() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ ([1, 2] | chain([3, 4]))[2] }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "3");
}
