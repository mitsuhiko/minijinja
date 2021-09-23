//! Built in filters and filter abstraction.
//!
//! This module implements the default filters which are registered in the
//! environment automatically.
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::sync::Arc;

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::utils::HtmlEscape;
use crate::value::{Value, ValueArgs};

type FilterFunc =
    dyn Fn(&Environment, Value, Vec<Value>) -> Result<Value, Error> + Sync + Send + 'static;

pub(crate) struct BoxedFilter(Arc<FilterFunc>);

/// A utility trait that represents filters.
pub trait Filter<V = Value, Rv = Value, Args = Vec<Value>>: Send + Sync + 'static {
    /// Applies a filter to value with the given arguments.
    fn apply_to(&self, env: &Environment, value: V, args: Args) -> Result<Rv, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, V, Rv, $($name),*> Filter<V, Rv, ($($name,)*)> for Func
        where
            Func: Fn(&Environment, V, $($name),*) -> Result<Rv, Error> + Send + Sync + 'static
        {
            fn apply_to(&self, env: &Environment, value: V, args: ($($name,)*)) -> Result<Rv, Error> {
                #[allow(non_snake_case)]
                let ($($name,)*) = args;
                (self)(env, value, $($name,)*)
            }
        }
    };
}

tuple_impls! {}
tuple_impls! { A }
tuple_impls! { A B }
tuple_impls! { A B C }
tuple_impls! { A B C D }

impl BoxedFilter {
    /// Creates a new boxed filter.
    pub fn new<F, V, Rv, Args>(f: F) -> BoxedFilter
    where
        F: Filter<V, Rv, Args>,
        V: TryFrom<Value>,
        Rv: Into<Value>,
        Args: ValueArgs,
    {
        BoxedFilter(Arc::new(move |env, value, args| -> Result<Value, Error> {
            f.apply_to(
                env,
                TryFrom::try_from(value).map_err(|_| {
                    Error::new(
                        ErrorKind::ImpossibleOperation,
                        "imcompatible value for filter",
                    )
                })?,
                ValueArgs::from_values(args)?,
            )
            .map(Into::into)
        }))
    }

    /// Applies the filter to a value and argument.
    pub fn apply_to(
        &self,
        env: &Environment,
        value: Value,
        args: Vec<Value>,
    ) -> Result<Value, Error> {
        (self.0)(env, value, args)
    }
}

/// Converts a value to uppercase.
pub fn upper(_env: &Environment, v: String) -> Result<String, Error> {
    Ok(v.to_uppercase())
}

/// Converts a value to lowercase.
pub fn lower(_env: &Environment, v: String) -> Result<String, Error> {
    Ok(v.to_lowercase())
}

/// Does a string replace.
pub fn replace(_env: &Environment, v: String, from: String, to: String) -> Result<String, Error> {
    Ok(v.replace(&from, &to))
}

/// Returns the "length" of the value
pub fn length(_env: &Environment, v: Value) -> Result<Value, Error> {
    v.len().map(Value::from).ok_or_else(|| {
        Error::new(
            ErrorKind::ImpossibleOperation,
            "cannot calculate length of this value",
        )
    })
}

/// Marks a value as safe.  This converts it into a string.
pub fn safe(_env: &Environment, v: String) -> Result<Value, Error> {
    // TODO: this ideally understands which type of escaping is in use
    Ok(Value::from_safe_string(v))
}

/// HTML escapes a string.
pub fn escape(_env: &Environment, v: Value) -> Result<Value, Error> {
    // TODO: this ideally understands which type of escaping is in use
    if v.is_safe() {
        Ok(v)
    } else {
        Ok(Value::from_safe_string(
            HtmlEscape(&v.to_string()).to_string(),
        ))
    }
}

pub(crate) fn get_default_filters() -> BTreeMap<&'static str, BoxedFilter> {
    let mut rv = BTreeMap::new();
    rv.insert("lower", BoxedFilter::new(lower));
    rv.insert("upper", BoxedFilter::new(upper));
    rv.insert("replace", BoxedFilter::new(replace));
    rv.insert("safe", BoxedFilter::new(safe));
    rv.insert("escape", BoxedFilter::new(escape));
    rv.insert("length", BoxedFilter::new(length));
    rv
}

#[test]
fn test_basics() {
    fn test(_: &Environment, a: u32, b: u32) -> Result<u32, Error> {
        Ok(a + b)
    }

    let env = Environment::new();
    let bx = BoxedFilter::new(test);
    assert_eq!(
        bx.apply_to(&env, Value::from(23), vec![Value::from(42)])
            .unwrap(),
        Value::from(65)
    );
}
