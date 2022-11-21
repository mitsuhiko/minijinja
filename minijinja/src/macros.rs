#[cfg(test)]
use similar_asserts::assert_eq;

// `ok!` and `some!` are less bloaty alternatives to the standard library's try operator (`?`).
// Since we do not need type conversions in this crate we can fall back to much easier match
// patterns that compile faster and produce less bloaty code.

macro_rules! ok {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => return Err(err),
        }
    };
}

macro_rules! some {
    ($expr:expr) => {
        match $expr {
            Some(val) => val,
            None => return None,
        }
    };
}

/// Hidden utility module for the [`context!`](crate::context!) macro.
#[doc(hidden)]
pub mod __context {
    use crate::key::Key;
    use crate::value::{MapType, Value, ValueMap, ValueRepr};
    use crate::Environment;
    use std::sync::Arc;

    #[inline(always)]
    pub fn make() -> ValueMap {
        ValueMap::default()
    }

    #[inline(always)]
    pub fn add(ctx: &mut ValueMap, key: &'static str, value: Value) {
        ctx.insert(Key::Str(key), value);
    }

    #[inline(always)]
    pub fn build(ctx: ValueMap) -> Value {
        ValueRepr::Map(Arc::new(ctx), MapType::Normal).into()
    }

    pub fn thread_local_env() -> Environment<'static> {
        thread_local! {
            static ENV: Environment<'static> = Environment::new()
        }
        ENV.with(|x| x.clone())
    }
}

/// Creates a template context with keys and values.
///
/// ```rust
/// # use minijinja::context;
/// let ctx = context! {
///     name => "Peter",
///     location => "World",
/// };
/// ```
///
/// Alternatively if the variable name matches the key name it can
/// be omitted:
///
/// ```rust
/// # use minijinja::context;
/// let name = "Peter";
/// let ctx = context! { name };
/// ```
///
/// The return value is a [`Value`](crate::value::Value).
///
/// Note that [`context!`](crate::context!) can also be used recursively if you need to
/// create nested objects:
///
/// ```rust
/// # use minijinja::context;
/// let ctx = context! {
///     nav => vec![
///         context!(path => "/", title => "Index"),
///         context!(path => "/downloads", title => "Downloads"),
///         context!(path => "/faq", title => "FAQ"),
///     ]
/// };
/// ```
#[macro_export]
macro_rules! context {
    () => {
        $crate::__context::build($crate::__context::make())
    };
    (
        $($key:ident $(=> $value:expr)?),* $(,)?
    ) => {{
        let mut ctx = $crate::__context::make();
        $(
            $crate::__context_pair!(ctx, $key $(, $value)?);
        )*
        $crate::__context::build(ctx)
    }}
}

#[macro_export]
#[doc(hidden)]
macro_rules! __context_pair {
    ($ctx:ident, $key:ident) => {{
        $crate::__context_pair!($ctx, $key, $key);
    }};
    ($ctx:ident, $key:ident, $value:expr) => {
        $crate::__context::add(
            &mut $ctx,
            stringify!($key),
            $crate::value::Value::from_serializable(&$value),
        );
    };
}

/// A macro similar to [`format!`] but that uses MiniJinja for rendering.
///
/// This can be used to quickly render a MiniJinja template into a string
/// without having to create an environment first which can be useful in
/// some situations.  Note however that the template is re-parsed every
/// time the [`render!`](crate::render) macro is called which is potentially
/// slow.
///
/// There are two forms for this macro.  The default form takes template
/// source and context variables, the extended form also lets you provide
/// a custom environment that should be used rather than a default one.
/// The context variables are passed the same way as with the
/// [`context!`](crate::context) macro.
///
/// # Example
///
/// Passing context explicitly:
///
/// ```
/// # use minijinja::render;
/// println!("{}", render!("Hello {{ name }}!", name => "World"));
/// ```
///
/// Passing variables with the default name:
///
/// ```
/// # use minijinja::render;
/// let name = "World";
/// println!("{}", render!("Hello {{ name }}!", name));
/// ```
///
/// Passing an explicit environment:
///
/// ```
/// # use minijinja::{Environment, render};
/// let env = Environment::new();
/// println!("{}", render!(in env, "Hello {{ name }}!", name => "World"));
/// ```
///
/// # Panics
///
/// This macro panics if the format string is an invalid template or the
/// template evaluation failed.
#[macro_export]
macro_rules! render {
    (
        in $env:expr,
        $tmpl:expr
        $(, $key:ident $(=> $value:expr)?)* $(,)?
    ) => {
        ($env).render_str($tmpl, $crate::context! { $($key $(=> $value)? ,)* })
            .expect("failed to render expression")
    };
    (
        $tmpl:expr
        $(, $key:ident $(=> $value:expr)?)* $(,)?
    ) => {
        $crate::render!(in $crate::__context::thread_local_env(), $tmpl, $($key $(=> $value)? ,)*)
    }
}

#[test]
fn test_context() {
    use crate::value::Value;
    let var1 = 23;
    let ctx = context!(var1, var2 => 42);
    assert_eq!(ctx.get_attr("var1").unwrap(), Value::from(23));
    assert_eq!(ctx.get_attr("var2").unwrap(), Value::from(42));
}

#[test]
fn test_render() {
    let env = crate::Environment::new();
    let rv = render!(in env, "Hello {{ name }}!", name => "World");
    assert_eq!(rv, "Hello World!");

    let rv = render!("Hello {{ name }}!", name => "World");
    assert_eq!(rv, "Hello World!");

    let rv = render!("Hello World!");
    assert_eq!(rv, "Hello World!");
}
