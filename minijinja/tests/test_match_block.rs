use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use minijinja::{render, Environment};

#[test]
fn test_match_arms() {
    let tmpl = "{% match x %}{% case 1 %}one{% case 2 %}two{% default %}other{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1), "one");
    assert_eq!(render!(tmpl, x => 2), "two");
    assert_eq!(render!(tmpl, x => 99), "other");
}

#[test]
fn test_match_strings() {
    let tmpl =
        r#"{% match color %}{% case "red" %}R{% case "green" %}G{% default %}?{% endmatch %}"#;
    assert_eq!(render!(tmpl, color => "green"), "G");
    assert_eq!(render!(tmpl, color => "yellow"), "?");
}

#[test]
fn test_match_booleans_and_floats() {
    let tmpl = "{% match flag %}{% case true %}yes{% case false %}no{% endmatch %}";
    assert_eq!(render!(tmpl, flag => true), "yes");
    assert_eq!(render!(tmpl, flag => false), "no");

    let tmpl = "{% match x %}{% case 3.14 %}pi{% default %}not pi{% endmatch %}";
    assert_eq!(render!(tmpl, x => 3.14), "pi");
}

#[test]
fn test_match_undefined_value() {
    let tmpl = r#"{% match x %}{% case "a" %}A{% default %}none{% endmatch %}"#;
    assert_eq!(render!(tmpl), "none");
}

#[test]
fn test_match_without_default() {
    let tmpl = "{% match x %}{% case 1 %}one{% case 2 %}two{% endmatch %}";
    assert_eq!(render!(tmpl, x => 2), "two");
    assert_eq!(render!(tmpl, x => 3), "");
}

#[test]
fn test_match_only_default() {
    let tmpl = "{% match x %}{% default %}always{% endmatch %}";
    assert_eq!(render!(tmpl, x => 42), "always");
}

#[test]
fn test_match_no_arms() {
    assert_eq!(render!("a{% match x %}{% endmatch %}b", x => 1), "ab");
}

#[test]
fn test_match_empty_arm_does_not_fall_through() {
    let tmpl = "{% match x %}{% case 1 %}{% default %}fallback{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1), "");
    assert_eq!(render!(tmpl, x => 2), "fallback");
}

#[test]
fn test_match_first_arm_wins() {
    let tmpl = "{% match x %}{% case 1 %}first{% case 1 %}second{% default %}default{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1), "first");
}

#[test]
fn test_match_expressions() {
    let tmpl = "{% match x + 1 %}{% case 2 %}yes{% default %}no{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1), "yes");
    assert_eq!(render!(tmpl, x => 2), "no");

    let tmpl = "{% match x %}{% case 1 + 1 %}yes{% default %}no{% endmatch %}";
    assert_eq!(render!(tmpl, x => 2), "yes");

    let tmpl = r#"{% match x|upper %}{% case "HELLO" %}yes{% default %}no{% endmatch %}"#;
    assert_eq!(render!(tmpl, x => "hello"), "yes");
}

#[test]
fn test_match_variable_pattern() {
    let tmpl = "{% match x %}{% case y %}same{% default %}different{% endmatch %}";
    assert_eq!(render!(tmpl, x => 42, y => 42), "same");
    assert_eq!(render!(tmpl, x => 42, y => 43), "different");
}

#[test]
fn test_match_evaluated_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut env = Environment::new();
    env.add_function("next", {
        let calls = calls.clone();
        move || calls.fetch_add(1, Ordering::SeqCst) + 1
    });
    let rv = env
        .render_str(
            "{% match next() %}{% case 5 %}a{% case 1 %}b{% case 1 %}c{% endmatch %}",
            (),
        )
        .unwrap();
    assert_eq!(rv, "b");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_match_multiple_patterns() {
    let tmpl =
        "{% match x %}{% case 1, 2, 3 %}low{% case 4, 5, 6 %}mid{% default %}high{% endmatch %}";
    assert_eq!(render!(tmpl, x => 2), "low");
    assert_eq!(render!(tmpl, x => 3), "low");
    assert_eq!(render!(tmpl, x => 5), "mid");
    assert_eq!(render!(tmpl, x => 10), "high");

    let tmpl = r#"{% match x %}{% case "a", "b", "c" %}abc{% default %}other{% endmatch %}"#;
    assert_eq!(render!(tmpl, x => "b"), "abc");
    assert_eq!(render!(tmpl, x => "d"), "other");
}

#[test]
fn test_match_guards() {
    let tmpl = "{% match x %}{% case 1 if y > 0 %}yes{% default %}no{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1, y => 5), "yes");
    assert_eq!(render!(tmpl, x => 1, y => -1), "no");
    assert_eq!(render!(tmpl, x => 2, y => 5), "no");
}

#[test]
fn test_match_guard_fallthrough() {
    let tmpl = "{% match x %}{% case 1 if y %}one-y{% case 1 %}one{% case 2 if z %}two-z{% default %}other{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1, y => true, z => false), "one-y");
    assert_eq!(render!(tmpl, x => 1, y => false, z => false), "one");
    assert_eq!(render!(tmpl, x => 2, y => false, z => true), "two-z");
    assert_eq!(render!(tmpl, x => 2, y => false, z => false), "other");
}

#[test]
fn test_match_multiple_patterns_with_guard() {
    let tmpl = "{% match x %}{% case 1, 2 if y %}hit{% default %}miss{% endmatch %}";
    assert_eq!(render!(tmpl, x => 2, y => true), "hit");
    assert_eq!(render!(tmpl, x => 2, y => false), "miss");
    assert_eq!(render!(tmpl, x => 3, y => true), "miss");
}

#[test]
fn test_match_guard_only_evaluated_after_pattern_match() {
    let tmpl = "{% match x %}{% case 1 if missing.attr %}a{% default %}b{% endmatch %}";
    assert_eq!(render!(tmpl, x => 2), "b");
}

#[test]
fn test_match_nested_statements() {
    let tmpl = "{% match x %}{% case 1 %}{% if y %}yes{% else %}no{% endif %}{% default %}other{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1, y => true), "yes");
    assert_eq!(render!(tmpl, x => 1, y => false), "no");

    let tmpl = "{% match x %}{% case 1 %}{% for i in items %}{{ i }}{% endfor %}{% default %}none{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1, items => vec![1, 2, 3]), "123");
    assert_eq!(render!(tmpl, x => 2, items => vec![1, 2, 3]), "none");
}

#[test]
fn test_match_inside_loop() {
    let tmpl = "{% for i in items %}{% match i %}{% case 1 %}a{% case 2 %}b{% default %}c{% endmatch %}{% endfor %}";
    assert_eq!(render!(tmpl, items => vec![2, 1, 3, 1]), "baca");
}

#[test]
fn test_match_nested_match() {
    let tmpl = r#"{% match x %}{% case 1 %}{% match y %}{% case "a" %}1a{% default %}1other{% endmatch %}{% default %}outer{% endmatch %}"#;
    assert_eq!(render!(tmpl, x => 1, y => "a"), "1a");
    assert_eq!(render!(tmpl, x => 1, y => "b"), "1other");
    assert_eq!(render!(tmpl, x => 2, y => "a"), "outer");
}

#[test]
fn test_match_surrounding_output() {
    let tmpl =
        "before {% match x %}{% case 1 %}[one]{% default %}[other]{% endmatch %} after {{ x }}";
    assert_eq!(render!(tmpl, x => 1), "before [one] after 1");
}

#[test]
fn test_match_set_inside_arm() {
    let tmpl = "{% match x %}{% case 1 %}{% set v = 'a' %}{% default %}{% set v = 'b' %}{% endmatch %}{{ v }}";
    assert_eq!(render!(tmpl, x => 1), "a");
    assert_eq!(render!(tmpl, x => 2), "b");
}

#[test]
fn test_match_whitespace_before_first_case() {
    let tmpl = "{% match x %}\n  {% case 1 %}one{% default %}other{% endmatch %}";
    assert_eq!(render!(tmpl, x => 1), "one");
}

#[test]
fn test_match_undeclared_variables() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str(
            "{% match a %}{% case b if c %}{{ d }}{% default %}{{ e }}{% endmatch %}",
        )
        .unwrap();
    let vars = tmpl.undeclared_variables(false);
    for name in ["a", "b", "c", "d", "e"] {
        assert!(vars.contains(name), "missing {name}");
    }
}

#[test]
fn test_match_syntax_errors() {
    let env = Environment::new();
    for source in [
        "{% match x %}{% case 1 %}one",
        "{% match %}{% endmatch %}",
        "{% match x %}{% case %}one{% endmatch %}",
        "{% match x %}stray{% case 1 %}one{% endmatch %}",
        "{% match x %}{% default %}a{% case 1 %}b{% endmatch %}",
    ] {
        assert!(
            env.template_from_str(source).is_err(),
            "expected syntax error for {source}"
        );
    }
}
