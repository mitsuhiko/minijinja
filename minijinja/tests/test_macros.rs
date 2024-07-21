#![cfg(feature = "macros")]
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use insta::assert_snapshot;
use similar_asserts::assert_eq;

use minijinja::value::{Kwargs, Object, Value};
use minijinja::{args, context, render, Environment, ErrorKind};

#[test]
fn test_context() {
    let var1 = 23;
    let ctx = context!(var1, var2 => 42);
    assert_eq!(ctx.get_attr("var1").unwrap(), Value::from(23));
    assert_eq!(ctx.get_attr("var2").unwrap(), Value::from(42));
}

#[test]
fn test_context_merge() {
    let one = context!(a => 1);
    let two = context!(b => 2, a => 42);
    let ctx = context![..one, ..two];
    assert_eq!(ctx.get_attr("a").unwrap(), Value::from(1));
    assert_eq!(ctx.get_attr("b").unwrap(), Value::from(2));

    let two = context!(b => 2, a => 42);
    let ctx = context!(a => 1, ..two);
    assert_eq!(ctx.get_attr("a").unwrap(), Value::from(1));
    assert_eq!(ctx.get_attr("b").unwrap(), Value::from(2));
}

#[test]
fn test_context_merge_custom() {
    #[derive(Debug, Clone)]
    struct X;

    impl Object for X {
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str()? {
                "a" => Some(Value::from(1)),
                "b" => Some(Value::from(2)),
                _ => None,
            }
        }
    }

    let x = Value::from_object(X);
    let ctx = context! { a => 42, ..x };

    assert_eq!(ctx.get_attr("a").unwrap(), Value::from(42));
    assert_eq!(ctx.get_attr("b").unwrap(), Value::from(2));
}

#[test]
fn test_render() {
    let env = Environment::new();
    let rv = render!(in env, "Hello {{ name }}!", name => "World");
    assert_eq!(rv, "Hello World!");

    let rv = render!("Hello {{ name }}!", name => "World");
    assert_eq!(rv, "Hello World!");

    let rv = render!("Hello World!");
    assert_eq!(rv, "Hello World!");
}

#[test]
fn test_args() {
    fn type_name_of_val<T: ?Sized>(_val: &T) -> &str {
        std::any::type_name::<T>()
    }

    let args = args!();
    assert_eq!(args.len(), 0);
    assert_eq!(type_name_of_val(args), "[minijinja::value::Value]");

    let args = args!(1, 2);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));
    assert_eq!(type_name_of_val(args), "[minijinja::value::Value]");

    let args = args!(1, 2,);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));

    let args = args!(1, 2, foo => 42, bar => 23);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));
    let kwargs = Kwargs::try_from(args[2].clone()).unwrap();
    assert_eq!(kwargs.get::<i32>("foo").unwrap(), 42);
    assert_eq!(kwargs.get::<i32>("bar").unwrap(), 23);

    let args = args!(1, 2, foo => 42, bar => 23,);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));
    let kwargs = Kwargs::try_from(args[2].clone()).unwrap();
    assert_eq!(kwargs.get::<i32>("foo").unwrap(), 42);
    assert_eq!(kwargs.get::<i32>("bar").unwrap(), 23);
    assert_eq!(type_name_of_val(args), "[minijinja::value::Value]");
}

#[test]
fn test_macro_passing() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{% macro m(a) %}{{ a }}{% endmacro %}")
        .unwrap();
    let (_, state) = tmpl.render_and_return_state(()).unwrap();
    let m = state.lookup("m").unwrap();
    assert_eq!(m.get_attr("name").unwrap().as_str(), Some("m"));
    let rv = m.call(&state, args!(42)).unwrap();
    assert_eq!(rv.as_str(), Some("42"));

    // if we call the macro on an empty state it errors
    let empty_state = env.empty_state();
    let err = m.call(&empty_state, args!(42)).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert_eq!(
        err.detail(),
        Some("cannot call this macro. template state went away.")
    );
}

#[test]
fn test_no_leak() {
    let dropped = Arc::new(AtomicBool::new(false));

    #[derive(Debug, Clone)]
    struct X(Arc<AtomicBool>);

    impl Object for X {
        fn get_value(self: &Arc<Self>, _name: &Value) -> Option<Value> {
            None
        }
    }

    impl Drop for X {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    let ctx = context! {
        x => Value::from_object(X(dropped.clone())),
    };
    let mut env = Environment::new();
    env.add_template("x", "{% macro meh() %}{{ x }}{{ meh }}{% endmacro %}")
        .unwrap();
    let rv = env
        .render_str(
            r#"
        {%- from 'x' import meh %}
        {{- meh() }}
        {%- set closure = x %}
        {%- macro foo() %}{{ foo }}{{ closure }}{% endmacro %}
        {{- foo() -}}

        {%- for y in range(3) %}
            {%- set closure = x %}
            {%- macro foo() %}{{ foo }}{{ closure }}{% endmacro %}
            {{- foo() -}}
        {%- endfor -%}
    "#,
            ctx,
        )
        .unwrap();

    assert!(dropped.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(
        rv,
        "{}<macro meh><macro foo>{}<macro foo>{}<macro foo>{}<macro foo>{}"
    );
}

/// https://github.com/mitsuhiko/minijinja/issues/434
#[test]
fn test_nested_macro_bug() {
    let rv = render!(
        r#"
    {% set a = 42 %}
    {% macro m1(var) -%}
      {{ var }}
    {%- endmacro %}

    {% macro m2(x=a) -%}
      {{ m1(x) }}
    {%- endmacro %}

    {{ m2() }}
    "#
    );
    assert_snapshot!(rv.trim(), @"42");
}

/// https://github.com/mitsuhiko/minijinja/issues/434
#[test]
fn test_caller_bug() {
    let rv = render!(
        r#"
    {% set a = 42 %}
    {% set b = 23 %}

    {% macro m1(var) -%}
      {{ caller(var) }}
    {%- endmacro %}

    {% macro m2(x=a) -%}
      {% call(var) m1(x) %}{{ var }}|{{ b }}{% endcall %}
    {%- endmacro %}

    {{ m2() }}
    "#
    );
    assert_snapshot!(rv.trim(), @"42|23");
}

/// https://github.com/mitsuhiko/minijinja/issues/535
#[test]
fn test_unenclosed_resolve() {
    // the current intended logic here is that a the state can
    // observe real globals and the initial template context, but
    // no other modifications.  Normally the call block can only
    // see what it encloses explicitly, but since it does not
    // refer to anything here it in fact has an empty closure.

    fn resolve(state: &minijinja::State, var: &str) -> Value {
        state.lookup(var).unwrap_or_default()
    }

    let mut env = Environment::new();
    env.add_global("ctx_global", "ctx global");
    env.add_function("resolve", resolve);
    let rv = env
        .render_str(
            r#"
    {%- set template_global = 'template global' %}
    {%- macro wrapper() %}{{ caller() }}{% endmacro %}
    {%- call wrapper() %}
        {{- resolve('render_global') }}|
        {{- resolve('ctx_global') }}|
        {{- resolve('template_global') }}
    {%- endcall -%}
    "#,
            context! { render_global => "render global" },
        )
        .unwrap();
    assert_snapshot!(rv, @"render global|ctx global|");
}
