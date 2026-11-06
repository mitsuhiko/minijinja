#![cfg(feature = "fuel")]
use minijinja::value::Value;
use minijinja::{context, Environment, ErrorKind, State};

#[test]
fn test_basic() {
    let mut env = Environment::new();
    assert_eq!(env.fuel(), None);
    env.set_fuel(Some(100));
    assert_eq!(env.fuel(), Some(100));
    env.add_template("test", "{% for x in seq %}{{ x }}\n{% endfor %}")
        .unwrap();
    let t = env.get_template("test").unwrap();

    // this will still manage to run with 100 fuel
    let rv = t
        .render(context!(seq => (0..15).collect::<Vec<_>>()))
        .unwrap();
    assert_eq!(rv.lines().count(), 15);

    // this is above the limit
    let rv = t
        .render(context!(seq => (0..20).collect::<Vec<_>>()))
        .unwrap_err();
    assert_eq!(rv.kind(), ErrorKind::OutOfFuel);
}

#[cfg(feature = "macros")]
#[test]
fn test_macro_fuel() {
    let mut env = Environment::new();
    assert_eq!(env.fuel(), None);
    env.set_fuel(Some(100));
    assert_eq!(env.fuel(), Some(100));
    env.add_template(
        "test",
        "
        {% macro x() %}{% for item in range(5) %}...{% endfor %}{% endmacro %}
        {% for count in range(macros) %}{{ x() }}{% endfor %}
    ",
    )
    .unwrap();
    let t = env.get_template("test").unwrap();

    // this should succeed
    t.render(context!(macros => 3)).unwrap();

    // but running more macros should not
    let err = t.render(context!(macros => 5)).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::OutOfFuel);
}

#[test]
fn test_output_consumes_fuel() {
    let mut env = Environment::new();
    env.set_fuel(Some(1000));
    // 64 bytes cost one fuel, so writing 64KB needs a lot more than the
    // handful of instructions involved.
    let err = env.render_str("{{ 'x' * 65536 }}", ()).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::OutOfFuel);

    let tmpl = env.template_from_str("{{ x }}").unwrap();
    let rv = tmpl
        .render_captured(context! { x => "x".repeat(6400) })
        .unwrap();
    let (consumed, _) = rv.state().fuel_levels().unwrap();
    assert!((100..110).contains(&consumed), "consumed {consumed}");
}

#[test]
fn test_size_is_checked_before_allocating() {
    let mut env = Environment::new();
    env.set_fuel(Some(1000));
    // The sizes fit into a 32-bit usize, as larger ones are rejected as out
    // of range there before fuel is checked.
    for source in [
        "{{ 'x' * 1000000000 }}",
        "{{ [1]|batch(1000000000, 'x') }}",
        "{{ 'a\\nb'|indent(1000000000) }}",
        "{{ '%1000000000s'|format('x') }}",
    ] {
        let err = env.render_str(source, ()).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::OutOfFuel, "{source}");
    }
}

#[test]
fn test_replace_count_limits_fuel() {
    let mut env = Environment::new();
    env.set_fuel(Some(1000));
    let source = "{{ ('a' * 1000)|replace('a', 'b' * 1000, count)|length }}";
    let rv = env.render_str(source, context! { count => 1 }).unwrap();
    assert_eq!(rv, "1999");
    let err = env
        .render_str(source, context! { count => -1 })
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::OutOfFuel);
}

#[test]
fn test_iteration_consumes_fuel() {
    let mut env = Environment::new();
    env.set_fuel(Some(10000));
    let infinite = Value::make_iterable(|| (0u64..).map(Value::from));
    for source in [
        "{{ items|list }}",
        "{{ items|join(',') }}",
        "{{ items|last }}",
        "{{ -1 in items }}",
        "{{ items[1000000000] }}",
    ] {
        let err = env
            .render_str(source, context! { items => infinite.clone() })
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::OutOfFuel, "{source}");
    }
}

#[test]
fn test_custom_consume_fuel() {
    let mut env = Environment::new();
    env.add_function("expensive", |state: &State, n: u64| {
        state.consume_fuel(n)?;
        Ok(n)
    });
    env.set_fuel(Some(100));
    assert_eq!(env.render_str("{{ expensive(50) }}", ()).unwrap(), "50");
    let err = env.render_str("{{ expensive(500) }}", ()).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::OutOfFuel);

    // without fuel this does nothing
    env.set_fuel(None);
    assert_eq!(env.render_str("{{ expensive(500) }}", ()).unwrap(), "500");
}
