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
    assert_eq!(rv, "[True]");
}

#[test]
fn test_state_temps() {
    fn inc(state: &mut State) -> Value {
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
fn test_state_extensions() {
    fn assert_send<T: Send>() {}
    assert_send::<State<'static, 'static>>();

    #[derive(Default)]
    struct Counter(usize);

    fn inc(state: &mut State) -> usize {
        let counter = state.get_or_insert_extension(Counter::default());
        counter.0 += 1;
        counter.0
    }

    fn current(state: &State) -> usize {
        state
            .get_extension::<Counter>()
            .map_or(0, |counter| counter.0)
    }

    let mut env = Environment::new();
    env.add_function("inc", inc);
    env.add_function("current", current);
    env.add_template("inc.txt", "{{ inc() }}").unwrap();
    let tmpl = env
        .template_from_str("{{ inc() }}|{% include 'inc.txt' %}|{{ current() }}|{{ inc() }}")
        .unwrap();
    let mut captured = tmpl.render_captured(()).unwrap();

    assert_eq!(captured.output(), "1|2|2|3");
    assert_eq!(captured.state().get_extension::<Counter>().unwrap().0, 3);
    captured.with_state_mut(|state| state.get_extension_mut::<Counter>().unwrap().0 += 1);
    assert_eq!(captured.state().get_extension::<Counter>().unwrap().0, 4);
    assert_eq!(
        captured.with_state_mut(|state| state.get_or_insert_extension(Counter(99)).0),
        4
    );
}

#[test]
fn test_state_object_temps() {
    #[derive(Debug, Default)]
    struct MyObject(AtomicUsize);

    impl Object for MyObject {}

    fn inc(state: &mut State) -> Value {
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

#[test]
fn test_known_variables() {
    let mut env = Environment::new();
    env.add_global("foo", 42);
    let state = env.empty_state();
    let mut vars = state.known_variables();
    vars.sort();
    assert_eq!(vars, vec!["debug", "dict", "foo", "namespace", "range"]);
}
