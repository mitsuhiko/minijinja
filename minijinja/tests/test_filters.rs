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

#[test]
fn test_zip_basic() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip(['a', 'b', 'c']) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, r#"[[1, "a"], [2, "b"], [3, "c"]]"#);
}

#[test]
fn test_zip_different_lengths() {
    let env = Environment::new();
    // Should stop at the shortest iterable
    let tmpl = env
        .template_from_str("{{ [1, 2] | zip(['a', 'b', 'c']) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, r#"[[1, "a"], [2, "b"]]"#);
}

#[test]
fn test_zip_multiple_iterables() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip(['a', 'b', 'c'], ['x', 'y', 'z']) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, r#"[[1, "a", "x"], [2, "b", "y"], [3, "c", "z"]]"#);
}

#[test]
fn test_zip_with_iteration() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{% for num, letter in [1, 2, 3] | zip(['a', 'b', 'c']) %}{{ num }}{{ letter }}{% endfor %}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "1a2b3c");
}

#[test]
fn test_zip_empty_list() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [] | zip([1, 2, 3]) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[]");
}

#[test]
fn test_zip_non_iterable_error() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip(42) | list }}")
        .unwrap();
    let err = tmpl.render(context!()).unwrap_err();
    assert!(err
        .to_string()
        .contains("zip filter argument must be iterable"));
}

#[test]
fn test_zip_single_iterable() {
    let env = Environment::new();
    // Zip with no additional arguments should return list of single-element tuples
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip() | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[[1], [2], [3]]");
}

#[test]
fn test_sort_attribute_list() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str(
            r"{{ [{'a': 1, 'b': 2, 'c': 5}, {'a': 2, 'b': 1, 'c': 6}] | sort(attribute='b,a') }}",
        )
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(
        result,
        r#"[{"a": 2, "b": 1, "c": 6}, {"a": 1, "b": 2, "c": 5}]"#
    );
}

#[test]
fn test_sort_attribute_list_reverse() {
    let env = Environment::new();
    let ctx = context! {
        cities => vec![
            context!(name => "Sydney", country => "Australia"),
            context!(name => "Sydney", country => "Canada"),
            context!(name => "Kochi", country => "India"),
            context!(name => "Kochi", country => "Japan"),
        ]
    };
    let tmpl = env
        .template_from_str(
            "{{ cities | sort(attribute='name, country', reverse=true) \
             | map(attribute='country')}}",
        )
        .unwrap();
    let result = tmpl.render(ctx).unwrap();
    assert_eq!(result, r#"["Canada", "Australia", "Japan", "India"]"#);
}

#[test]
fn test_sort_attribute_list_single() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str(r"{{ [{'a': 1, 'b': 2}, {'a': 2, 'b': 1}] | sort(attribute='b,') }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, r#"[{"a": 2, "b": 1}, {"a": 1, "b": 2}]"#);
}

#[test]
fn test_sort_attribute_stable_reverse() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str(
            "{{ [[0, 1], [1, 1], [3, 2], [5, 2]] \
            | sort(attribute='1', reverse=true) }}",
        )
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, r#"[[3, 2], [5, 2], [0, 1], [1, 1]]"#);
}
