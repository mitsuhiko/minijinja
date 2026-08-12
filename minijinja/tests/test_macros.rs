#![cfg(feature = "macros")]
#[cfg(feature = "multi_template")]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(feature = "serde")]
use insta::assert_debug_snapshot;
use insta::assert_snapshot;
#[cfg(feature = "serde")]
use serde::Serialize;
use similar_asserts::assert_eq;

use minijinja::value::{Kwargs, Object, Value};
use minijinja::{args, context, render, Environment, Error, ErrorKind, State};

#[test]
fn test_context() {
    let var1 = 23;
    let ctx = context!(var1, var2 => 42);
    assert_eq!(ctx.get_attr("var1").unwrap(), Value::from(23));
    assert_eq!(ctx.get_attr("var2").unwrap(), Value::from(42));
}

#[test]
#[cfg(feature = "preserve_order")]
fn test_context_preserves_order() {
    let ctx = context!(zebra => 1, apple => 2);
    let keys = ctx
        .try_iter()
        .unwrap()
        .map(|key| key.to_string())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["zebra", "apple"]);
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
    let mut rendered = tmpl.render_captured(()).unwrap();
    let m = rendered.state().lookup("m").unwrap();
    assert_eq!(m.get_attr("name").unwrap().as_str(), Some("m"));
    let rv = rendered
        .with_state_mut(|state| m.call(state, args!(42)))
        .unwrap();
    assert_eq!(rv.as_str(), Some("42"));

    // if we call the macro on an empty state it errors
    let mut empty_state = env.empty_state();
    let err = m.call(&mut empty_state, args!(42)).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert_eq!(
        err.detail(),
        Some("cannot call this macro. template state went away.")
    );
}

#[cfg(feature = "multi_template")]
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

#[test]
fn test_nested_macro_can_escape_invocation() {
    let rv = render!(
        r#"
        {%- set first = namespace() -%}
        {%- set second = namespace() -%}
        {%- macro outer(target, value) -%}
            {%- macro inner() -%}{{ value }}{%- endmacro -%}
            {%- set target.inner = inner -%}
        {%- endmacro -%}
        {{- outer(first, "first") -}}
        {{- outer(second, "second") -}}
        {{- first.inner() }}|{{ second.inner() -}}
        "#
    );
    assert_eq!(rv, "first|second");
}

#[test]
fn test_nested_macro_can_escape_into_state() {
    fn stash(state: &mut State, value: Value) -> &'static str {
        state.set_temp("macro", value);
        ""
    }

    fn call_stashed(state: &mut State) -> Result<Value, Error> {
        let value = state.get_temp("macro").unwrap();
        value.call(state, &[])
    }

    let mut env = Environment::new();
    env.add_function("stash", stash);
    env.add_function("call_stashed", call_stashed);
    let rv = env
        .render_str(
            r#"
            {%- macro outer(value) -%}
                {%- macro inner() -%}{{ value }}{%- endmacro -%}
                {{- stash(inner) -}}
            {%- endmacro -%}
            {{- outer("captured") -}}
            {{- call_stashed() -}}
            "#,
            (),
        )
        .unwrap();
    assert_eq!(rv, "captured");
}

#[cfg(feature = "multi_template")]
#[test]
fn test_escaped_macro_survives_failed_invocation() {
    #[derive(Default)]
    struct Calls(usize);

    fn stash(state: &mut State, value: Value) -> Result<&'static str, Error> {
        state.set_temp("macro", value);
        let calls = state.get_or_insert_extension(Calls::default());
        calls.0 += 1;
        if calls.0 == 1 {
            Ok("")
        } else {
            Err(Error::new(ErrorKind::InvalidOperation, "boom"))
        }
    }

    let mut env = Environment::new();
    env.add_function("stash", stash);
    let mut captured = env
        .template_from_str(
            r#"
            {%- block body -%}
                {%- macro outer(value) -%}
                    {%- macro inner() -%}{{ value }}{%- endmacro -%}
                    {{- stash(inner) -}}
                {%- endmacro -%}
                {{- outer("captured") -}}
            {%- endblock -%}
            "#,
        )
        .unwrap()
        .render_captured(())
        .unwrap();

    captured
        .with_state_mut(|state| state.render_block("body"))
        .unwrap_err();
    let value = captured.state().get_temp("macro").unwrap();
    let rv = captured
        .with_state_mut(|state| value.call(state, &[]))
        .unwrap();
    assert_eq!(rv.as_str(), Some("captured"));
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

#[test]
fn test_macro_state_sees_enclosed_variables() {
    let mut env = Environment::new();
    env.add_function("is_known", |state: &State, name: &str| {
        state
            .known_variables()
            .iter()
            .any(|variable| variable == name)
    });
    let rv = env
        .render_str(
            "{% set foo = 42 %}{% macro test() %}{{ foo }}:{{ is_known('foo') }}{% endmacro %}{{ test() }}",
            (),
        )
        .unwrap();
    assert_eq!(rv, "42:True");
}

#[cfg(feature = "multi_template")]
#[test]
fn test_macro_callbacks_can_render_blocks() {
    fn render_block(state: &mut State, name: &str) -> Result<String, Error> {
        state.render_block(name)
    }

    let mut env = Environment::new();
    env.add_function("render_block", render_block);
    let rv = env
        .render_str(
            "{% macro invoke() %}{{ render_block('body') }}{% endmacro %}{% block body %}body{% endblock %}|{{ invoke() }}|{{ render_block('body') }}",
            (),
        )
        .unwrap();

    assert_eq!(rv, "body|body|body");
}

#[test]
fn test_macro_debug_includes_enclosed_variables() {
    let env = Environment::new();
    let rv = env
        .render_str(
            "{% set captured = 'needle' %}{% macro test() %}{{ captured }}|{{ debug() }}{% endmacro %}{{ test() }}",
            (),
        )
        .unwrap();

    assert!(rv.contains("\"captured\": 'needle'"), "{rv}");
}

#[test]
fn test_macro_reuses_mutable_state() {
    #[derive(Default)]
    struct CallCount(usize);

    #[derive(Debug, Default)]
    struct StateProbe(AtomicUsize);

    impl Object for StateProbe {
        fn call(
            self: &Arc<Self>,
            state: &mut State<'_, '_>,
            _args: &[Value],
        ) -> Result<Value, Error> {
            let ptr = state as *mut State<'_, '_> as usize;
            match self
                .0
                .compare_exchange(0, ptr, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => {}
                Err(previous) => assert_eq!(previous, ptr),
            }
            let count = state.get_or_insert_extension(CallCount::default());
            count.0 += 1;
            Ok(Value::from(count.0))
        }
    }

    let mut env = Environment::new();
    env.add_global("probe", Value::from_object(StateProbe::default()));
    let rv = env
        .render_str(
            "{{ probe() }}{% macro test() %}{{ probe() }}{% endmacro %}{{ test() }}",
            (),
        )
        .unwrap();
    assert_eq!(rv, "12");
}

#[cfg(feature = "serde")]
#[test]
fn test_conversions() {
    struct SerializeOnly;
    struct FromOnly;
    struct Both;

    impl Serialize for SerializeOnly {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str("serialize-only")
        }
    }

    impl From<FromOnly> for Value {
        fn from(_: FromOnly) -> Self {
            Value::from("from-only")
        }
    }

    impl From<Both> for Value {
        fn from(_: Both) -> Self {
            Value::from("both")
        }
    }

    impl Serialize for Both {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str("SHOULD NEVER SHOW UP")
        }
    }

    let value = context! {
        both => Both,
        from_only => FromOnly,
        serialize_only => minijinja::value::Serde(SerializeOnly),
    };
    assert_debug_snapshot!(&value, @"
    {
        'both': 'both',
        'from_only': 'from-only',
        'serialize_only': 'serialize-only',
    }
    ");
}
