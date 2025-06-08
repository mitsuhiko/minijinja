use minijinja::{context, Environment};

#[test]
fn test_pycompat_rendering_boolean_values() {
    let mut env = Environment::new();

    env.add_template("bool_true", "{{ true }}").unwrap();
    env.add_template("bool_false", "{{ false }}").unwrap();

    // Test default rendering
    env.set_pycompat_rendering(false);
    let true_default = env
        .get_template("bool_true")
        .unwrap()
        .render(context! {})
        .unwrap();
    let false_default = env
        .get_template("bool_false")
        .unwrap()
        .render(context! {})
        .unwrap();

    assert_eq!(true_default, "true");
    assert_eq!(false_default, "false");

    // Test pycompat rendering
    env.set_pycompat_rendering(true);
    let true_pycompat = env
        .get_template("bool_true")
        .unwrap()
        .render(context! {})
        .unwrap();
    let false_pycompat = env
        .get_template("bool_false")
        .unwrap()
        .render(context! {})
        .unwrap();

    assert_eq!(true_pycompat, "True");
    assert_eq!(false_pycompat, "False");
}

#[test]
fn test_pycompat_rendering_none_value() {
    let mut env = Environment::new();

    env.add_template("none_val", "{{ none }}").unwrap();

    // Test default rendering
    env.set_pycompat_rendering(false);
    let none_default = env
        .get_template("none_val")
        .unwrap()
        .render(context! {})
        .unwrap();

    assert_eq!(none_default, "none");

    // Test pycompat rendering
    env.set_pycompat_rendering(true);
    let none_pycompat = env
        .get_template("none_val")
        .unwrap()
        .render(context! {})
        .unwrap();

    assert_eq!(none_pycompat, "None");
}

#[test]
fn test_pycompat_rendering_array() {
    let mut env = Environment::new();

    env.add_template("array", "{{ [true, false, none] }}")
        .unwrap();

    // Test default rendering
    env.set_pycompat_rendering(false);
    let array_default = env
        .get_template("array")
        .unwrap()
        .render(context! {})
        .unwrap();

    assert_eq!(array_default, "[true, false, none]");

    // Test pycompat rendering
    env.set_pycompat_rendering(true);
    let array_pycompat = env
        .get_template("array")
        .unwrap()
        .render(context! {})
        .unwrap();

    assert_eq!(array_pycompat, "[True, False, None]");
}

#[test]
fn test_pycompat_rendering_complex_case() {
    let mut env = Environment::new();

    // Test the specific case from the issue: {{ [true, false, none, 'foo', "bar'baz", '\x13'] }}
    env.add_template("complex", "{{ [true, false, none, 'foo'] }}")
        .unwrap();

    // Test default rendering
    env.set_pycompat_rendering(false);
    let _complex_default = env
        .get_template("complex")
        .unwrap()
        .render(context! {})
        .unwrap();

    // Test pycompat rendering
    env.set_pycompat_rendering(true);
    let complex_pycompat = env
        .get_template("complex")
        .unwrap()
        .render(context! {})
        .unwrap();

    // Check that pycompat has Python-style values
    assert!(complex_pycompat.contains("True"));
    assert!(complex_pycompat.contains("False"));
    assert!(complex_pycompat.contains("None"));
    assert!(complex_pycompat.contains("'foo'"));
}

#[test]
fn test_pycompat_rendering_exact_github_case() {
    let mut env = Environment::new();

    // Test the exact case from the GitHub issue
    env.add_template(
        "github_case",
        r#"{{ [true, false, none, 'foo', "bar'baz", '\x13'] }}"#,
    )
    .unwrap();

    // Test pycompat rendering
    env.set_pycompat_rendering(true);
    let result = env
        .get_template("github_case")
        .unwrap()
        .render(context! {})
        .unwrap();

    println!("GitHub case result: {}", result);

    // Check that pycompat has Python-style values
    assert!(result.contains("True"));
    assert!(result.contains("False"));
    assert!(result.contains("None"));
    assert!(result.contains("'foo'"));
    // Check for proper string quoting and escaping
    assert!(result.contains("\"bar'baz\"") || result.contains("'bar\\'baz'"));
    assert!(result.contains("'\\x13'"));
}
