use minijinja::value::Value;
use minijinja::{Environment, Error, ErrorKind, State};

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
fn test_formatter_can_modify_state() {
    #[derive(Default)]
    struct FormatCount(usize);

    let mut env = Environment::new();
    env.set_formatter(|out, state, value| {
        state.get_or_insert_extension(FormatCount::default()).0 += 1;
        minijinja::escape_formatter(out, state, value)
    });
    let captured = env
        .template_from_str("{{ 1 }}|{{ 2 }}")
        .unwrap()
        .render_captured(())
        .unwrap();
    assert_eq!(captured.output(), "1|2");
    assert_eq!(
        captured.state().get_extension::<FormatCount>().unwrap().0,
        2
    );
}

#[test]
fn test_render_block_restores_state_after_error() {
    #[derive(Default)]
    struct Calls(usize);

    fn fail_after_first(state: &mut State) -> Result<&'static str, Error> {
        let calls = state.get_or_insert_extension(Calls::default());
        calls.0 += 1;
        if calls.0 == 1 {
            Ok("ok")
        } else {
            Err(Error::new(ErrorKind::InvalidOperation, "boom"))
        }
    }

    let mut env = Environment::new();
    env.add_function("fail_after_first", fail_after_first);
    let mut captured = env
        .template_from_str(
            "{% block bad %}{% set leaked = 'yes' %}{% for x in [1] %}{{ fail_after_first() }}{% endfor %}{% endblock %}{% block good %}{{ leaked|default('clean') }}{% endblock %}",
        )
        .unwrap()
        .render_captured(())
        .unwrap();
    assert_eq!(captured.output(), "okclean");

    let err = captured
        .with_state_mut(|state| state.render_block("bad"))
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert_eq!(captured.state().current_block(), None);
    assert_eq!(captured.state().lookup("leaked"), None);
    assert_eq!(captured.state().get_extension::<Calls>().unwrap().0, 2);
    assert_eq!(
        captured
            .with_state_mut(|state| state.render_block("good"))
            .unwrap(),
        "clean"
    );
}

#[test]
fn test_render_block_restores_state_after_setup_error() {
    let mut env = Environment::new();
    env.set_recursion_limit(0);
    env.add_global("global", 42);
    let tmpl = env
        .template_from_str("{% block test %}unreachable{% endblock %}")
        .unwrap();
    let mut state = tmpl.new_state();
    let original_name = state.name().to_owned();

    let err = state.render_block("test").unwrap_err();
    assert_eq!(err.detail(), Some("recursion limit exceeded"));
    assert_eq!(state.name(), original_name);
    assert_eq!(state.current_block(), None);
    assert_eq!(state.lookup("global"), Some(Value::from(42)));
}

#[test]
fn test_include_restores_state_after_setup_error() {
    #[derive(Default)]
    struct Calls(usize);

    fn include_after_first(state: &mut State) -> bool {
        let calls = state.get_or_insert_extension(Calls::default());
        calls.0 += 1;
        calls.0 > 1
    }

    let mut env = Environment::new();
    env.set_recursion_limit(2);
    env.add_function("include_after_first", include_after_first);
    env.add_template("included", "included").unwrap();
    env.add_template(
        "main",
        "{% block bad %}{% if include_after_first() %}{% include 'included' %}{% endif %}{% endblock %}{% block good %}clean{% endblock %}",
    )
    .unwrap();
    let mut captured = env
        .get_template("main")
        .unwrap()
        .render_captured(())
        .unwrap();
    assert_eq!(captured.output(), "clean");

    let err = captured
        .with_state_mut(|state| state.render_block("bad"))
        .unwrap_err();
    assert_eq!(err.detail(), Some("recursion limit exceeded"));
    assert_eq!(captured.state().name(), "main");
    assert_eq!(captured.state().current_block(), None);
    assert_eq!(
        captured
            .with_state_mut(|state| state.render_block("good"))
            .unwrap(),
        "clean"
    );
}

#[test]
fn test_super_restores_state_after_setup_error() {
    #[derive(Default)]
    struct Calls(usize);

    fn super_on_second(state: &mut State) -> bool {
        let calls = state.get_or_insert_extension(Calls::default());
        calls.0 += 1;
        calls.0 == 2
    }

    let mut env = Environment::new();
    env.set_recursion_limit(2);
    env.add_function("super_on_second", super_on_second);
    env.add_template("parent", "{% block body %}parent{% endblock %}")
        .unwrap();
    env.add_template(
        "child",
        "{% extends 'parent' %}{% block body %}{% if super_on_second() %}{{ super() }}{% endif %}{% endblock %}",
    )
    .unwrap();
    let mut captured = env
        .get_template("child")
        .unwrap()
        .render_captured(())
        .unwrap();
    assert_eq!(captured.output(), "");

    let err = captured
        .with_state_mut(|state| state.render_block("body"))
        .unwrap_err();
    assert_eq!(err.detail(), Some("recursion limit exceeded"));
    assert_eq!(captured.state().current_block(), None);
    assert_eq!(
        captured
            .with_state_mut(|state| state.render_block("body"))
            .unwrap(),
        ""
    );
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
