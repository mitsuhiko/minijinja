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
    let state = env.empty_state();
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
    assert_eq!(rv, "true");

    let rv = env
        .template_from_str("{{ 42 is foo . bar . baz }}")
        .unwrap()
        .render(())
        .unwrap();
    assert_eq!(rv, "true");
}
