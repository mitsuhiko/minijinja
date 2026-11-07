use similar_asserts::assert_eq;

use minijinja::{args, Environment, State};

#[test]
fn test_basics() {
    fn test(_: &State, a: u32, b: u32) -> bool {
        assert_eq!(a, 23);
        a == b
    }

    let mut env = Environment::new();
    env.add_test("test", test);
    let mut state = env.empty_state();
    assert!(state.perform_test("test", args!(23, 23)).unwrap());
}

#[test]
fn test_dotted_test_name() {
    let mut env = Environment::new();
    env.add_test("foo.bar.baz", |value: i32| value == 42);

    let rv = env
        .template_from_str("{{ 42 is foo.bar.baz }}")
        .unwrap()
        .render(())
        .unwrap();
    assert_eq!(rv, "True");

    let rv = env
        .template_from_str("{{ 42 is foo . bar . baz }}")
        .unwrap()
        .render(())
        .unwrap();
    assert_eq!(rv, "True");
}

#[test]
#[cfg(feature = "builtins")]
fn test_number_tests_integer_range() {
    let env = Environment::new();
    let render = |src: &str| env.render_str(src, ()).unwrap();

    // dividing by zero is not divisible rather than a panic
    assert_eq!(render("{{ 5 is divisibleby 0 }}"), "False");
    assert_eq!(
        render("{{ 340282366920938463463374607431768211455 is divisibleby 0 }}"),
        "False"
    );
    // i128::MIN % -1 overflows an i128
    assert_eq!(
        render("{{ (-170141183460469231731687303715884105727 - 1) is divisibleby -1 }}"),
        "True"
    );
    // integers above i128::MAX
    assert_eq!(
        render("{{ 340282366920938463463374607431768211455 is divisibleby 5 }}"),
        "True"
    );
    assert_eq!(
        render("{{ 340282366920938463463374607431768211455 is divisibleby 340282366920938463463374607431768211455 }}"),
        "True"
    );
    assert_eq!(
        render("{{ 340282366920938463463374607431768211455 is divisibleby 170141183460469231731687303715884105728 }}"),
        "False"
    );
    assert_eq!(
        render("{{ 340282366920938463463374607431768211455 is odd }}"),
        "True"
    );
    assert_eq!(
        render("{{ 340282366920938463463374607431768211455 is even }}"),
        "False"
    );
    assert_eq!(
        render("{{ 340282366920938463463374607431768211454 is even }}"),
        "True"
    );
}
