use similar_asserts::assert_eq;

use minijinja::filters::indent;

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
