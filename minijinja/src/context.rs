#[cfg(test)]
use similar_asserts::assert_eq;

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
/// Note that [`context!`] can also be used recursively if you need to
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
    (
        $($key:ident $(=> $value:expr)?),* $(,)?
    ) => {{
        let mut ctx = std::collections::BTreeMap::default();
        $(
            $crate::__pair!(ctx, $key $(, $value)?);
        )*
        $crate::value::Value::from(ctx)
    }}
}

#[macro_export]
#[doc(hidden)]
macro_rules! __pair {
    ($ctx:ident, $key:ident) => {{
        $crate::__pair!($ctx, $key, $key);
    }};
    ($ctx:ident, $key:ident, $value:expr) => {
        $ctx.insert(
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
