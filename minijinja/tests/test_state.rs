use std::sync::atomic::{AtomicUsize, Ordering};

use minijinja::value::{Object, Value};
use minijinja::{Environment, State};

#[test]
fn test_state_lookup_global() {
    let mut env = Environment::new();
    env.add_function("lookup_global", |state: &State| -> Value {
        state.lookup("the_global").unwrap_or_default()
    });
    env.add_global("the_global", true);
    let rv = env.render_str("[{{ lookup_global() }}]", ()).unwrap();
    assert_eq!(rv, "[true]");
}

#[test]
fn test_state_temps() {
    fn inc(state: &State) -> Value {
        let old = state
            .get_temp("my_counter")
            .unwrap_or_else(|| Value::from(0i64));
        let new = Value::from(i64::try_from(old).unwrap() + 1);
        state.set_temp("my_counter", new.clone());
        new
    }

    let mut env = Environment::new();
    env.add_function("inc", inc);
    env.add_template("inc.txt", "{{ inc() }}").unwrap();
    let rv = env
        .render_str(
            "{{ inc() }}|{% include 'inc.txt' %}|{% if true %}{{ inc() }}{% endif %}",
            (),
        )
        .unwrap();
    assert_eq!(rv, "1|2|3");
}

#[test]
fn test_state_object_temps() {
    #[derive(Debug, Default)]
    struct MyObject(AtomicUsize);

    impl Object for MyObject {}

    fn inc(state: &State) -> Value {
        let obj = state.get_or_set_temp_object("my_counter", MyObject::default);
        let old = obj.0.fetch_add(1, Ordering::AcqRel);
        Value::from(old + 1)
    }

    let mut env = Environment::new();
    env.add_function("inc", inc);
    env.add_template("inc.txt", "{{ inc() }}").unwrap();
    let rv = env
        .render_str(
            "{{ inc() }}|{% include 'inc.txt' %}|{% if true %}{{ inc() }}{% endif %}",
            (),
        )
        .unwrap();
    assert_eq!(rv, "1|2|3");
}
