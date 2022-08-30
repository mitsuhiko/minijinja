#[cfg(test)]
use similar_asserts::assert_eq;

/// Hidden utility module for the [`context!`](crate::context!) macro.
#[doc(hidden)]
pub mod __context {
    use crate::key::Key;
    use crate::value::{RcType, Value, ValueMap, ValueRepr};

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
        ValueRepr::Map(RcType::new(ctx)).into()
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

#[test]
fn test_macro() {
    use crate::value::Value;
    let var1 = 23;
    let ctx = context!(var1, var2 => 42);
    assert_eq!(ctx.get_attr("var1").unwrap(), Value::from(23));
    assert_eq!(ctx.get_attr("var2").unwrap(), Value::from(42));
}
