use similar_asserts::assert_eq;

use minijinja::value::Value;
use minijinja::{Environment, State};

#[test]
fn test_basics() {
    fn test(_: &State, a: u32, b: u32) -> bool {
        assert_eq!(a, 23);
        a == b
    }

    let mut env = Environment::new();
    env.add_test("test", test);
    let state = env.empty_state();
    assert!(state
        .perform_test("test", &[Value::from(23), Value::from(23)][..])
        .unwrap());
}
