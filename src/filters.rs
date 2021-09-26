//! Filter functions and abstractions.
//!
//! MiniJinja inherits from Jinja2 the concept of filter functions.  These are functions
//! which are applied to values to modify them.  For example the expression `{{ 42|filter(23) }}`
//! invokes the filter `filter` with the arguments `42` and `23`.
//!
//! MiniJinja comes with some built-in filters that are listed below. To create a
//! custom filter write a function that takes at least an
//! [`&Environment`](crate::Environment) and value argument, then register it
//! with [`add_filter`](crate::Environment::add_filter).
//!
//! ## Custom Filters
//!
//! A custom filter is just a simple function which accepts inputs as parameters and then
//! returns a new value.  For instance the following shows a filter which takes an input
//! value and replaces whitespace with dashes and converts it to lowercase:
//!
//! ```
//! # use minijinja::{Environment, Error};
//! # let mut env = Environment::new();
//! fn slugify(env: &Environment, value: String) -> Result<String, Error> {
//!     Ok(value.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-"))
//! }
//!
//! env.add_filter("slugify", slugify);
//! ```
//!
//! MiniJinja will perform the necessary conversions automatically via the
//! [`FunctionArgs`](crate::value::FunctionArgs) and [`Into`] traits.
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::environment::Environment;
use crate::error::Error;
use crate::value::{ArgType, FunctionArgs, Value};

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
        V: ArgType,
        Rv: Into<Value>,
        Args: FunctionArgs,
    {
        BoxedFilter(Arc::new(move |env, value, args| -> Result<Value, Error> {
            f.apply_to(
                env,
                ArgType::from_value(Some(value))?,
                FunctionArgs::from_values(args)?,
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

pub(crate) fn get_builtin_filters() -> BTreeMap<&'static str, BoxedFilter> {
    #[allow(unused_mut)]
    let mut rv = BTreeMap::new();
    #[cfg(feature = "builtin_filters")]
    {
        rv.insert("lower", BoxedFilter::new(lower));
        rv.insert("upper", BoxedFilter::new(upper));
        rv.insert("replace", BoxedFilter::new(replace));
        rv.insert("safe", BoxedFilter::new(safe));
        rv.insert("escape", BoxedFilter::new(escape));
        rv.insert("length", BoxedFilter::new(length));
        rv.insert("dictsort", BoxedFilter::new(dictsort));
        rv.insert("reverse", BoxedFilter::new(reverse));
        rv.insert("trim", BoxedFilter::new(trim));
        rv.insert("join", BoxedFilter::new(join));
    }
    rv
}

#[cfg(feature = "builtin_filters")]
mod builtins {
    use super::*;

    use crate::error::ErrorKind;
    use crate::utils::HtmlEscape;
    use crate::value::{Primitive, ValueKind};
    use std::cmp::Ordering;
    use std::fmt::Write;

    /// Converts a value to uppercase.
    pub fn upper(_env: &Environment, v: String) -> Result<String, Error> {
        Ok(v.to_uppercase())
    }

    /// Converts a value to lowercase.
    pub fn lower(_env: &Environment, v: String) -> Result<String, Error> {
        Ok(v.to_lowercase())
    }

    /// Does a string replace.
    pub fn replace(
        _env: &Environment,
        v: String,
        from: String,
        to: String,
    ) -> Result<String, Error> {
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

    /// Dict sorting functionality.
    pub fn dictsort(_env: &Environment, v: Value) -> Result<Value, Error> {
        let mut pairs = v.try_into_pairs()?;
        pairs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        Ok(Value::from(pairs))
    }

    /// Reverses a list or string
    pub fn reverse(_env: &Environment, v: Value) -> Result<Value, Error> {
        if let Some(Primitive::Str(s)) = v.as_primitive() {
            Ok(Value::from(s.chars().rev().collect::<String>()))
        } else if matches!(v.kind(), ValueKind::Seq) {
            let mut v = v.try_into_vec()?;
            v.reverse();
            Ok(Value::from(v))
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot reverse this value",
            ))
        }
    }

    /// Trims a value
    pub fn trim(_env: &Environment, s: String, chars: Option<String>) -> Result<String, Error> {
        match chars {
            Some(chars) => {
                let chars = chars.chars().collect::<Vec<_>>();
                Ok(s.trim_matches(&chars[..]).to_string())
            }
            None => Ok(s.trim().to_string()),
        }
    }

    /// Joins a sequence by a character
    pub fn join(_env: &Environment, val: Value, joiner: Option<String>) -> Result<String, Error> {
        if val.is_undefined() || val.is_none() {
            return Ok(String::new());
        }

        let joiner = joiner.as_ref().map_or("", |x| x.as_str());

        if let Some(Primitive::Str(s)) = val.as_primitive() {
            let mut rv = String::new();
            for c in s.chars() {
                if !rv.is_empty() {
                    rv.push_str(joiner);
                }
                rv.push(c);
            }
            Ok(rv)
        } else if matches!(val.kind(), ValueKind::Seq) {
            let mut rv = String::new();
            for item in val.try_into_vec()? {
                if !rv.is_empty() {
                    rv.push_str(joiner);
                }
                if let Some(s) = item.as_str() {
                    rv.push_str(s);
                } else {
                    write!(rv, "{}", item).ok();
                }
            }
            Ok(rv)
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot join this value",
            ))
        }
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

    #[test]
    fn test_optional_args() {
        fn add(_: &Environment, val: u32, a: u32, b: Option<u32>) -> Result<u32, Error> {
            let mut sum = val + a;
            if let Some(b) = b {
                sum += b;
            }
            Ok(sum)
        }

        let env = Environment::new();
        let bx = BoxedFilter::new(add);
        assert_eq!(
            bx.apply_to(&env, Value::from(23), vec![Value::from(42)])
                .unwrap(),
            Value::from(65)
        );
        assert_eq!(
            bx.apply_to(
                &env,
                Value::from(23),
                vec![Value::from(42), Value::UNDEFINED]
            )
            .unwrap(),
            Value::from(65)
        );
        assert_eq!(
            bx.apply_to(&env, Value::from(23), vec![Value::from(42), Value::from(1)])
                .unwrap(),
            Value::from(66)
        );
    }
}

#[cfg(feature = "builtin_filters")]
pub use self::builtins::*;
