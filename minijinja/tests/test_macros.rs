#![cfg(feature = "macros")]
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use similar_asserts::assert_eq;

use minijinja::value::{Kwargs, StructObject, Value};
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
    struct X;
    impl StructObject for X {
        fn get_field(&self, name: &str) -> Option<Value> {
            match name {
                "a" => Some(Value::from(1)),
                "b" => Some(Value::from(2)),
                _ => None,
            }
        }
    }

    let x = Value::from_struct_object(X);
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

    struct X(Arc<AtomicBool>);

    impl StructObject for X {
        fn get_field(&self, _name: &str) -> Option<Value> {
            None
        }
    }

    impl Drop for X {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    let ctx = context! {
        x => Value::from_struct_object(X(dropped.clone())),
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
