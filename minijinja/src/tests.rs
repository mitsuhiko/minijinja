//! Test functions and abstractions.
//!
//! Test functions in MiniJinja are like [`filters`](crate::filters) but a
//! different syntax is used to invoke them and they have to return boolean
//! values.  For instance the expression `{% if foo is odd %}` invokes the
//! [`is_odd`] test to check if the value is indeed an odd number.
//!
//! MiniJinja comes with some built-in test functions that are listed below. To
//! create a custom test write a function that takes at least a
//! [`&State`](crate::State) and value argument and returns a boolean
//! result, then register it with [`add_filter`](crate::Environment::add_test).
//!
//! # Using Tests
//!
//! Tests are useful to "test" a value in a specific way.  For instance if
//! you want to assign different classes to alternating rows one way is
//! using the `odd` test:
//!
//! ```jinja
//! {% if seq is defined %}
//!   <ul>
//!   {% for item in seq %}
//!     <li class="{{ 'even' if loop.index is even else 'odd' }}">{{ item }}</li>
//!   {% endfor %}
//!   </ul>
//! {% endif %}
//! ```
//!
//! # Custom Tests
//!
//! A custom test function is just a simple function which accepts [`State`] and
//! inputs as parameters and then returns a bool. For instance the following
//! shows a test function which takes an input value and checks if it's
//! lowercase:
//!
//! ```
//! # use minijinja::Environment;
//! # let mut env = Environment::new();
//! use minijinja::State;
//!
//! fn is_lowercase(_state: &State, value: String) -> bool {
//!     value.chars().all(|x| x.is_lowercase())
//! }
//!
//! env.add_test("lowercase", is_lowercase);
//! ```
//!
//! MiniJinja will perform the necessary conversions automatically.  For more
//! information see the [`Test`] trait.
//!
//! # Built-in Tests
//!
//! When the `builtins` feature is enabled a range of built-in tests are
//! automatically added to the environment.  These are also all provided in
//! this module.  Note though that these functions are not to be
//! called from Rust code as their exact interface (arguments and return types)
//! might change from one MiniJinja version to another.
use std::sync::Arc;

use crate::error::Error;
use crate::utils::SealedMarker;
use crate::value::{ArgType, FunctionArgs, Value};
use crate::vm::State;

type TestFunc = dyn Fn(&State, &Value, &[Value]) -> Result<bool, Error> + Sync + Send + 'static;

#[derive(Clone)]
pub(crate) struct BoxedTest(Arc<TestFunc>);

/// A utility trait that represents the return value of filters.
///
/// It's implemented for the following types:
///
/// * `bool`
/// * `Result<bool, Error>`
///
/// The equivalent for filters or functions is [`FunctionResult`](crate::value::FunctionResult).
pub trait TestResult {
    #[doc(hidden)]
    fn into_result(self) -> Result<bool, Error>;
}

impl TestResult for Result<bool, Error> {
    fn into_result(self) -> Result<bool, Error> {
        self
    }
}

impl TestResult for bool {
    fn into_result(self) -> Result<bool, Error> {
        Ok(self)
    }
}

/// A utility trait that represents test functions.
///
/// This trait is used by the [`add_test`](crate::Environment::add_test) method to abstract over
/// different types of functions that implement tests.  Tests are similar to
/// [`filters`](crate::filters) but they always return boolean values and use a
/// slightly different syntax to filters.  Like filters they accept the [`State`] by
/// reference as first parameter and the value that that the test is applied to as second.
/// Additionally up to 4 further parameters are supported.
///
/// A test function can return any of the following types:
///
/// * `bool`
/// * `Result<bool, Error>`
///
/// Tests accept one mandatory parameter which is the value the filter is
/// applied to and up to 4 extra parameters.  The extra parameters can be
/// marked optional by using `Option<T>`.  All types are supported for which
/// [`ArgType`] is implemented.
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::State;
///
/// fn is_lowercase(_state: &State, value: String) -> bool {
///     value.chars().all(|x| x.is_lowercase())
/// }
///
/// env.add_test("lowercase", is_lowercase);
/// ```
///
/// For a list of built-in tests see [`tests`](crate::tests).
pub trait Test<V, Rv, Args>: Send + Sync + 'static {
    /// Performs a test to value with the given arguments.
    #[doc(hidden)]
    fn perform(&self, state: &State, value: V, args: Args, _: SealedMarker) -> Rv;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, V, Rv, $($name),*> Test<V, Rv, ($($name,)*)> for Func
        where
            Func: Fn(&State, V, $($name),*) -> Rv + Send + Sync + 'static,
            V: for<'a> ArgType<'a>,
            Rv: TestResult,
            $($name: for<'a> ArgType<'a>),*
        {
            fn perform(&self, state: &State, value: V, args: ($($name,)*), _: SealedMarker) -> Rv {
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
    pub fn new<F, V, Rv, Args>(f: F) -> BoxedTest
    where
        F: Test<V, Rv, Args>,
        V: for<'a> ArgType<'a>,
        Rv: TestResult,
        Args: for<'a> FunctionArgs<'a>,
    {
        BoxedTest(Arc::new(move |state, value, args| -> Result<bool, Error> {
            let value = Some(value);
            f.perform(
                state,
                ArgType::from_value(value)?,
                FunctionArgs::from_values(args)?,
                SealedMarker,
            )
            .into_result()
        }))
    }

    /// Applies the filter to a value and argument.
    pub fn perform(&self, state: &State, value: &Value, args: &[Value]) -> Result<bool, Error> {
        (self.0)(state, value, args)
    }
}

#[cfg(feature = "builtins")]
mod builtins {
    use super::*;

    use std::convert::TryFrom;

    use crate::value::ValueKind;

    /// Checks if a value is odd.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_odd(_state: &State, v: Value) -> bool {
        i128::try_from(v).ok().map_or(false, |x| x % 2 != 0)
    }

    /// Checks if a value is even.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_even(_state: &State, v: Value) -> bool {
        i128::try_from(v).ok().map_or(false, |x| x % 2 == 0)
    }

    /// Checks if a value is undefined.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_undefined(_state: &State, v: Value) -> bool {
        v.is_undefined()
    }

    /// Checks if a value is defined.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_defined(_state: &State, v: Value) -> bool {
        !v.is_undefined()
    }

    /// Checks if this value is a number.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_number(_state: &State, v: Value) -> bool {
        matches!(v.kind(), ValueKind::Number)
    }

    /// Checks if this value is a string.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_string(_state: &State, v: Value) -> bool {
        matches!(v.kind(), ValueKind::String)
    }

    /// Checks if this value is a sequence
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_sequence(_state: &State, v: Value) -> bool {
        matches!(v.kind(), ValueKind::Seq)
    }

    /// Checks if this value is a mapping
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_mapping(_state: &State, v: Value) -> bool {
        matches!(v.kind(), ValueKind::Map)
    }

    /// Checks if the value is starting with a string.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_startingwith(_state: &State, v: String, other: String) -> bool {
        v.starts_with(&other)
    }

    /// Checks if the value is ending with a string.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_endingwith(_state: &State, v: String, other: String) -> bool {
        v.ends_with(&other)
    }

    #[test]
    fn test_basics() {
        fn test(_: &State, a: u32, b: u32) -> bool {
            a == b
        }

        let env = crate::Environment::new();
        State::with_dummy(&env, |state| {
            let bx = BoxedTest::new(test);
            assert!(bx
                .perform(state, &Value::from(23), &[Value::from(23)][..])
                .unwrap());
        });
    }
}

#[cfg(feature = "builtins")]
pub use self::builtins::*;
