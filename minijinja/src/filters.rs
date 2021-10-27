//! Filter functions and abstractions.
//!
//! MiniJinja inherits from Jinja2 the concept of filter functions.  These are functions
//! which are applied to values to modify them.  For example the expression `{{ 42|filter(23) }}`
//! invokes the filter `filter` with the arguments `42` and `23`.
//!
//! MiniJinja comes with some built-in filters that are listed below. To create a
//! custom filter write a function that takes at least a
//! [`&State`](crate::State) and value argument, then register it
//! with [`add_filter`](crate::Environment::add_filter).
//!
//! ## Custom Filters
//!
//! A custom filter is just a simple function which accepts inputs as parameters and then
//! returns a new value.  For instance the following shows a filter which takes an input
//! value and replaces whitespace with dashes and converts it to lowercase:
//!
//! ```
//! # use minijinja::{Environment, State, Error};
//! # let mut env = Environment::new();
//! fn slugify(_state: &State, value: String) -> Result<String, Error> {
//!     Ok(value.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-"))
//! }
//!
//! env.add_filter("slugify", slugify);
//! ```
//!
//! MiniJinja will perform the necessary conversions automatically via the
//! [`FunctionArgs`](crate::value::FunctionArgs) and [`Into`] traits.
use std::collections::BTreeMap;

use crate::error::Error;
use crate::utils::HtmlEscape;
use crate::value::{ArgType, FunctionArgs, RcType, Value};
use crate::vm::State;

type FilterFunc = dyn Fn(&State, Value, Vec<Value>) -> Result<Value, Error> + Sync + Send + 'static;

#[derive(Clone)]
pub(crate) struct BoxedFilter(RcType<FilterFunc>);

/// A utility trait that represents filters.
pub trait Filter<V = Value, Rv = Value, Args = Vec<Value>>: Send + Sync + 'static {
    /// Applies a filter to value with the given arguments.
    fn apply_to(&self, state: &State, value: V, args: Args) -> Result<Rv, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, V, Rv, $($name),*> Filter<V, Rv, ($($name,)*)> for Func
        where
            Func: Fn(&State, V, $($name),*) -> Result<Rv, Error> + Send + Sync + 'static
        {
            fn apply_to(&self, state: &State, value: V, args: ($($name,)*)) -> Result<Rv, Error> {
                #[allow(non_snake_case)]
                let ($($name,)*) = args;
                (self)(state, value, $($name,)*)
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
        BoxedFilter(RcType::new(
            move |state, value, args| -> Result<Value, Error> {
                f.apply_to(
                    state,
                    ArgType::from_value(Some(value))?,
                    FunctionArgs::from_values(args)?,
                )
                .map(Into::into)
            },
        ))
    }

    /// Applies the filter to a value and argument.
    pub fn apply_to(&self, state: &State, value: Value, args: Vec<Value>) -> Result<Value, Error> {
        (self.0)(state, value, args)
    }
}

pub(crate) fn get_builtin_filters() -> BTreeMap<&'static str, BoxedFilter> {
    let mut rv = BTreeMap::new();
    rv.insert("safe", BoxedFilter::new(safe));
    rv.insert("escape", BoxedFilter::new(escape));
    rv.insert("e", BoxedFilter::new(escape));
    #[cfg(feature = "builtin_filters")]
    {
        rv.insert("lower", BoxedFilter::new(lower));
        rv.insert("upper", BoxedFilter::new(upper));
        rv.insert("replace", BoxedFilter::new(replace));
        rv.insert("length", BoxedFilter::new(length));
        rv.insert("count", BoxedFilter::new(length));
        rv.insert("dictsort", BoxedFilter::new(dictsort));
        rv.insert("reverse", BoxedFilter::new(reverse));
        rv.insert("trim", BoxedFilter::new(trim));
        rv.insert("join", BoxedFilter::new(join));
        rv.insert("default", BoxedFilter::new(default));
        rv.insert("d", BoxedFilter::new(default));
    }
    rv
}

/// Marks a value as safe.  This converts it into a string.
pub fn safe(_state: &State, v: String) -> Result<Value, Error> {
    // TODO: this ideally understands which type of escaping is in use
    Ok(Value::from_safe_string(v))
}

/// HTML escapes a string.
///
/// By default this filter is also registered under the alias `e`.
pub fn escape(_state: &State, v: Value) -> Result<Value, Error> {
    // TODO: this ideally understands which type of escaping is in use
    if v.is_safe() {
        Ok(v)
    } else {
        Ok(Value::from_safe_string(
            HtmlEscape(&v.to_string()).to_string(),
        ))
    }
}

#[cfg(feature = "builtin_filters")]
mod builtins {
    use super::*;

    use crate::error::ErrorKind;
    use crate::utils::matches;
    use crate::value::{Primitive, ValueKind};
    use std::cmp::Ordering;
    use std::fmt::Write;

    /// Converts a value to uppercase.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn upper(_state: &State, v: String) -> Result<String, Error> {
        Ok(v.to_uppercase())
    }

    /// Converts a value to lowercase.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn lower(_state: &State, v: String) -> Result<String, Error> {
        Ok(v.to_lowercase())
    }

    /// Does a string replace.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn replace(_state: &State, v: String, from: String, to: String) -> Result<String, Error> {
        Ok(v.replace(&from, &to))
    }

    /// Returns the "length" of the value
    ///
    /// By default this filter is also registered under the alias `count`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn length(_state: &State, v: Value) -> Result<Value, Error> {
        v.len().map(Value::from).ok_or_else(|| {
            Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot calculate length of this value",
            )
        })
    }

    /// Dict sorting functionality.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn dictsort(_state: &State, v: Value) -> Result<Value, Error> {
        let mut pairs = v.try_into_pairs()?;
        pairs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        Ok(Value::from(pairs))
    }

    /// Reverses a list or string
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn reverse(_state: &State, v: Value) -> Result<Value, Error> {
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
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn trim(_state: &State, s: String, chars: Option<String>) -> Result<String, Error> {
        match chars {
            Some(chars) => {
                let chars = chars.chars().collect::<Vec<_>>();
                Ok(s.trim_matches(&chars[..]).to_string())
            }
            None => Ok(s.trim().to_string()),
        }
    }

    /// Joins a sequence by a character
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn join(_state: &State, val: Value, joiner: Option<String>) -> Result<String, Error> {
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

    /// Checks if a string starts with another string.
    ///
    /// By default this filter is also registered under the alias `d`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_filters")))]
    pub fn default(_: &State, value: Value, other: Option<Value>) -> Result<Value, Error> {
        Ok(if value.is_undefined() {
            other.unwrap_or_else(|| Value::from(""))
        } else {
            value
        })
    }

    #[test]
    fn test_basics() {
        fn test(_: &State, a: u32, b: u32) -> Result<u32, Error> {
            Ok(a + b)
        }

        let env = crate::Environment::new();
        let ctx = crate::vm::Context::default();
        let state = State::from_env_and_context(&env, &ctx);
        let bx = BoxedFilter::new(test);
        assert_eq!(
            bx.apply_to(&state, Value::from(23), vec![Value::from(42)])
                .unwrap(),
            Value::from(65)
        );
    }

    #[test]
    fn test_optional_args() {
        fn add(_: &State, val: u32, a: u32, b: Option<u32>) -> Result<u32, Error> {
            let mut sum = val + a;
            if let Some(b) = b {
                sum += b;
            }
            Ok(sum)
        }

        let env = crate::Environment::new();
        let ctx = crate::vm::Context::default();
        let state = State::from_env_and_context(&env, &ctx);
        let bx = BoxedFilter::new(add);
        assert_eq!(
            bx.apply_to(&state, Value::from(23), vec![Value::from(42)])
                .unwrap(),
            Value::from(65)
        );
        assert_eq!(
            bx.apply_to(
                &state,
                Value::from(23),
                vec![Value::from(42), Value::UNDEFINED]
            )
            .unwrap(),
            Value::from(65)
        );
        assert_eq!(
            bx.apply_to(
                &state,
                Value::from(23),
                vec![Value::from(42), Value::from(1)]
            )
            .unwrap(),
            Value::from(66)
        );
    }
}

#[cfg(feature = "builtin_filters")]
pub use self::builtins::*;
