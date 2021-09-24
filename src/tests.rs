//! Test functions and abstractions.
//!
//! Test functions in MiniJinja are like (filters)[crate::filters] but a different syntax
//! is used to invoke them and they have to return boolean values.  For instance the
//! expression `{% if foo is odd %}` invokes the [`is_odd`] test to check if the value
//! is indeed an odd number.
//!
//! MiniJinja comes with some built-in test functions that are listed below. To
//! create a custom test write a function that takes at least an
//! [`&Environment`](crate::Environment) and value argument and returns a boolean
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
//! # use minijinja::{Environment, Error};
//! # let mut env = Environment::new();
//! fn is_lowercase(&env: Environment, value: String) -> Result<bool, Error> {
//!    Ok(value.is_lowercase())
//! }
//!
//! env.add_test("lowercase", is_lowercase);
//! ```
//!
//! MiniJinja will perform the necessary conversions automatically via the
//! [`ValueArgs`](crate::value::ValueArgs) trait.
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::sync::Arc;

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::value::{Value, ValueArgs};

type TestFunc =
    dyn Fn(&Environment, Value, Vec<Value>) -> Result<bool, Error> + Sync + Send + 'static;

pub(crate) struct BoxedTest(Arc<TestFunc>);

/// A utility trait that represents filters.
pub trait Test<V = Value, Args = Vec<Value>>: Send + Sync + 'static {
    /// Performs a test to value with the given arguments.
    fn perform(&self, env: &Environment, value: V, args: Args) -> Result<bool, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, V, $($name),*> Test<V, ($($name,)*)> for Func
        where
            Func: Fn(&Environment, V, $($name),*) -> Result<bool, Error> + Send + Sync + 'static
        {
            fn perform(&self, env: &Environment, value: V, args: ($($name,)*)) -> Result<bool, Error> {
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

impl BoxedTest {
    /// Creates a new boxed filter.
    pub fn new<F, V, Args>(f: F) -> BoxedTest
    where
        F: Test<V, Args>,
        V: TryFrom<Value>,
        Args: ValueArgs,
    {
        BoxedTest(Arc::new(move |env, value, args| -> Result<bool, Error> {
            f.perform(
                env,
                TryFrom::try_from(value).map_err(|_| {
                    Error::new(
                        ErrorKind::ImpossibleOperation,
                        "imcompatible value for filter",
                    )
                })?,
                ValueArgs::from_values(args)?,
            )
        }))
    }

    /// Applies the filter to a value and argument.
    pub fn perform(
        &self,
        env: &Environment,
        value: Value,
        args: Vec<Value>,
    ) -> Result<bool, Error> {
        (self.0)(env, value, args)
    }
}

/// Checks if a value is odd.
pub fn is_odd(_env: &Environment, v: Value) -> Result<bool, Error> {
    Ok(v.as_primitive()
        .and_then(|x| x.as_i128())
        .map_or(false, |x| x % 2 != 0))
}

/// Checks if a value is even.
pub fn is_even(_env: &Environment, v: Value) -> Result<bool, Error> {
    Ok(v.as_primitive()
        .and_then(|x| x.as_i128())
        .map_or(false, |x| x % 2 == 0))
}

/// Checks if a value is undefined.
pub fn is_undefined(_env: &Environment, v: Value) -> Result<bool, Error> {
    Ok(v.is_undefined())
}

/// Checks if a value is defined.
pub fn is_defined(_env: &Environment, v: Value) -> Result<bool, Error> {
    Ok(!v.is_undefined())
}

pub(crate) fn get_default_tests() -> BTreeMap<&'static str, BoxedTest> {
    let mut rv = BTreeMap::new();
    rv.insert("odd", BoxedTest::new(is_odd));
    rv.insert("even", BoxedTest::new(is_even));
    rv.insert("undefined", BoxedTest::new(is_undefined));
    rv.insert("defined", BoxedTest::new(is_defined));
    rv
}

#[test]
fn test_basics() {
    fn test(_: &Environment, a: u32, b: u32) -> Result<bool, Error> {
        Ok(a == b)
    }

    let env = Environment::new();
    let bx = BoxedTest::new(test);
    assert!(bx
        .perform(&env, Value::from(23), vec![Value::from(23)])
        .unwrap());
}
