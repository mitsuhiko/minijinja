//! Test functions and abstractions.
//!
//! Test functions in MiniJinja are like (filters)[crate::filters] but a different syntax
//! is used to invoke them and they have to return boolean values.  For instance the
//! expression `{% if foo is odd %}` invokes the [`is_odd`] test to check if the value
//! is indeed an odd number.
//!
//! MiniJinja comes with some built-in test functions that are listed below. To
//! create a custom test write a function that takes at least a
//! [`&State`](crate::State) and value argument and returns a boolean
//! result, then register it with [`add_filter`](crate::Environment::add_test).
//!
//! ## Custom Tests
//!
//! A custom test function is just a simple function which accepts inputs as
//! parameters and then returns a bool wrapped in a result. For instance the
//! following shows a test function which takes an input value and checks if
//! it's lowercase:
//!
//! ```
//! # use minijinja::{State, Environment, Error};
//! # let mut env = Environment::new();
//! fn is_lowercase(_state: &State, value: String) -> Result<bool, Error> {
//!    Ok(value.chars().all(|x| x.is_lowercase()))
//! }
//!
//! env.add_test("lowercase", is_lowercase);
//! ```
//!
//! MiniJinja will perform the necessary conversions automatically via the
//! [`FunctionArgs`](crate::value::FunctionArgs) trait.
use std::collections::BTreeMap;

use crate::error::Error;
use crate::value::{ArgType, FunctionArgs, RcType, Value};
use crate::vm::State;

type TestFunc = dyn Fn(&State, Value, Vec<Value>) -> Result<bool, Error> + Sync + Send + 'static;

#[derive(Clone)]
pub(crate) struct BoxedTest(RcType<TestFunc>);

/// A utility trait that represents filters.
pub trait Test<V = Value, Args = Vec<Value>>: Send + Sync + 'static {
    /// Performs a test to value with the given arguments.
    fn perform(&self, state: &State, value: V, args: Args) -> Result<bool, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, V, $($name),*> Test<V, ($($name,)*)> for Func
        where
            Func: Fn(&State, V, $($name),*) -> Result<bool, Error> + Send + Sync + 'static
        {
            fn perform(&self, state: &State, value: V, args: ($($name,)*)) -> Result<bool, Error> {
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

impl BoxedTest {
    /// Creates a new boxed filter.
    pub fn new<F, V, Args>(f: F) -> BoxedTest
    where
        F: Test<V, Args>,
        V: ArgType,
        Args: FunctionArgs,
    {
        BoxedTest(RcType::new(
            move |state, value, args| -> Result<bool, Error> {
                f.perform(
                    state,
                    ArgType::from_value(Some(value))?,
                    FunctionArgs::from_values(args)?,
                )
            },
        ))
    }

    /// Applies the filter to a value and argument.
    pub fn perform(&self, state: &State, value: Value, args: Vec<Value>) -> Result<bool, Error> {
        (self.0)(state, value, args)
    }
}

pub(crate) fn get_builtin_tests() -> BTreeMap<&'static str, BoxedTest> {
    #[allow(unused_mut)]
    let mut rv = BTreeMap::new();
    #[cfg(feature = "builtin_tests")]
    {
        rv.insert("odd", BoxedTest::new(is_odd));
        rv.insert("even", BoxedTest::new(is_even));
        rv.insert("undefined", BoxedTest::new(is_undefined));
        rv.insert("defined", BoxedTest::new(is_defined));
        rv.insert("number", BoxedTest::new(is_number));
        rv.insert("string", BoxedTest::new(is_string));
        rv.insert("sequence", BoxedTest::new(is_sequence));
        rv.insert("mapping", BoxedTest::new(is_mapping));
        rv.insert("startingwith", BoxedTest::new(is_startingwith));
        rv.insert("endingwith", BoxedTest::new(is_endingwith));
    }
    rv
}

#[cfg(feature = "builtin_tests")]
mod builtins {
    use super::*;

    use crate::utils::matches;
    use crate::value::ValueKind;

    /// Checks if a value is odd.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_odd(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(v.as_primitive()
            .and_then(|x| x.as_i128())
            .map_or(false, |x| x % 2 != 0))
    }

    /// Checks if a value is even.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_even(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(v.as_primitive()
            .and_then(|x| x.as_i128())
            .map_or(false, |x| x % 2 == 0))
    }

    /// Checks if a value is undefined.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_undefined(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(v.is_undefined())
    }

    /// Checks if a value is defined.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_defined(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(!v.is_undefined())
    }

    /// Checks if this value is a number.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_number(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(matches!(v.kind(), ValueKind::Number))
    }

    /// Checks if this value is a string.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_string(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(matches!(v.kind(), ValueKind::String))
    }

    /// Checks if this value is a sequence
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_sequence(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(matches!(v.kind(), ValueKind::Seq))
    }

    /// Checks if this value is a mapping
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_mapping(_state: &State, v: Value) -> Result<bool, Error> {
        Ok(matches!(v.kind(), ValueKind::Map))
    }

    /// Checks if the value is starting with a string.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_startingwith(_state: &State, v: String, other: String) -> Result<bool, Error> {
        Ok(v.starts_with(&other))
    }

    /// Checks if the value is ending with a string.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_tests")))]
    pub fn is_endingwith(_state: &State, v: String, other: String) -> Result<bool, Error> {
        Ok(v.ends_with(&other))
    }

    #[test]
    fn test_basics() {
        fn test(_: &State, a: u32, b: u32) -> Result<bool, Error> {
            Ok(a == b)
        }

        let env = crate::Environment::new();
        let ctx = crate::vm::Context::default();
        let state = State::from_env_and_context(&env, &ctx);
        let bx = BoxedTest::new(test);
        assert!(bx
            .perform(&state, Value::from(23), vec![Value::from(23)])
            .unwrap());
    }
}

#[cfg(feature = "builtin_tests")]
pub use self::builtins::*;
