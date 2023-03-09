//! Test functions and abstractions.
//!
//! Test functions in MiniJinja are like [`filters`](crate::filters) but a
//! different syntax is used to invoke them and they have to return boolean
//! values.  For instance the expression `{% if foo is defined %}` invokes the
//! [`is_defined`] test to check if the value is indeed an odd number.
//!
//! MiniJinja comes with some built-in test functions that are listed below. To
//! create a custom test write a function that takes at least a value argument
//! that returns a boolean result, then register it with
//! [`add_filter`](crate::Environment::add_test).
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
//! A custom test function is just a simple function which accepts its
//! inputs as parameters and then returns a bool. For instance the following
//! shows a test function which takes an input value and checks if it's
//! lowercase:
//!
//! ```
//! # use minijinja::Environment;
//! # let mut env = Environment::new();
//! fn is_lowercase(value: String) -> bool {
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

type TestFunc = dyn Fn(&State, &[Value]) -> Result<bool, Error> + Sync + Send + 'static;

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
/// marked optional by using `Option<T>`.  The last argument can also use
/// [`Rest<T>`](crate::value::Rest) to capture the remaining arguments.  All
/// types are supported for which [`ArgType`] is implemented.
///
/// For a list of built-in tests see [`tests`](crate::tests).
///
/// # Basic Example
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::State;
///
/// fn is_lowercase(value: String) -> bool {
///     value.chars().all(|x| x.is_lowercase())
/// }
///
/// env.add_test("lowercase", is_lowercase);
/// ```
///
/// ```jinja
/// {{ "foo" is lowercase }} -> true
/// ```
///
/// # Arguments and Optional Arguments
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::State;
///
/// fn is_containing(value: String, other: String) -> bool {
///     value.contains(&other)
/// }
///
/// env.add_test("containing", is_containing);
/// ```
///
/// ```jinja
/// {{ "foo" is containing("o") }} -> true
/// ```
pub trait Test<Rv, Args>: Send + Sync + 'static {
    /// Performs a test to value with the given arguments.
    #[doc(hidden)]
    fn perform(&self, args: Args, _: SealedMarker) -> Rv;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, Rv, $($name),*> Test<Rv, ($($name,)*)> for Func
        where
            Func: Fn($($name),*) -> Rv + Send + Sync + 'static,
            Rv: TestResult,
            $($name: for<'a> ArgType<'a>),*
        {
            fn perform(&self, args: ($($name,)*), _: SealedMarker) -> Rv {
                #[allow(non_snake_case)]
                let ($($name,)*) = args;
                (self)($($name,)*)
            }
        }
    };
}

tuple_impls! {}
tuple_impls! { A }
tuple_impls! { A B }
tuple_impls! { A B C }
tuple_impls! { A B C D }
tuple_impls! { A B C D E }

impl BoxedTest {
    /// Creates a new boxed filter.
    pub fn new<F, Rv, Args>(f: F) -> BoxedTest
    where
        F: Test<Rv, Args> + for<'a> Test<Rv, <Args as FunctionArgs<'a>>::Output>,
        Rv: TestResult,
        Args: for<'a> FunctionArgs<'a>,
    {
        BoxedTest(Arc::new(move |state, args| -> Result<bool, Error> {
            f.perform(ok!(Args::from_values(Some(state), args)), SealedMarker)
                .into_result()
        }))
    }

    /// Applies the filter to a value and argument.
    pub fn perform(&self, state: &State, args: &[Value]) -> Result<bool, Error> {
        (self.0)(state, args)
    }
}

/// Checks if a value is undefined.
///
/// ```jinja
/// {{ 42 is undefined }} -> false
/// ```
pub fn is_undefined(v: Value) -> bool {
    v.is_undefined()
}

/// Checks if a value is defined.
///
/// ```jinja
/// {{ 42 is defined }} -> true
/// ```
pub fn is_defined(v: Value) -> bool {
    !v.is_undefined()
}

/// Checks if a value is none.
///
/// ```jinja
/// {{ none is none }} -> true
/// ```
pub fn is_none(v: Value) -> bool {
    v.is_none()
}

/// Checks if a value is safe.
///
/// ```jinja
/// {{ "<hello>"|escape is safe }} -> true
/// ```
///
/// This filter is also registered with the `escaped` alias.
pub fn is_safe(v: Value) -> bool {
    v.is_safe()
}

#[cfg(feature = "builtins")]
mod builtins {
    use super::*;

    use std::borrow::Cow;
    use std::convert::TryFrom;

    use crate::value::ValueKind;

    /// Checks if a value is odd.
    ///
    /// ```jinja
    /// {{ 41 is odd }} -> true
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_odd(v: Value) -> bool {
        i128::try_from(v).ok().map_or(false, |x| x % 2 != 0)
    }

    /// Checks if a value is even.
    ///
    /// ```jinja
    /// {{ 42 is even }} -> true
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_even(v: Value) -> bool {
        i128::try_from(v).ok().map_or(false, |x| x % 2 == 0)
    }

    /// Checks if this value is a number.
    ///
    /// ```jinja
    /// {{ 42 is number }} -> true
    /// {{ "42" is number }} -> false
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_number(v: Value) -> bool {
        matches!(v.kind(), ValueKind::Number)
    }

    /// Checks if this value is a string.
    ///
    /// ```jinja
    /// {{ "42" is string }} -> true
    /// {{ 42 is string }} -> false
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_string(v: Value) -> bool {
        matches!(v.kind(), ValueKind::String)
    }

    /// Checks if this value is a sequence
    ///
    /// ```jinja
    /// {{ [1, 2, 3] is sequence }} -> true
    /// {{ 42 is sequence }} -> false
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_sequence(v: Value) -> bool {
        matches!(v.kind(), ValueKind::Seq)
    }

    /// Checks if this value is a mapping
    ///
    /// ```jinja
    /// {{ {"foo": "bar"} is mapping }} -> true
    /// {{ [1, 2, 3] is mapping }} -> false
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_mapping(v: Value) -> bool {
        matches!(v.kind(), ValueKind::Map)
    }

    /// Checks if the value is starting with a string.
    ///
    /// ```jinja
    /// {{ "foobar" is startingwith "foo" }} -> true
    /// {{ "foobar" is startingwith "bar" }} -> false
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_startingwith(v: Cow<'_, str>, other: Cow<'_, str>) -> bool {
        v.starts_with(&other as &str)
    }

    /// Checks if the value is ending with a string.
    ///
    /// ```jinja
    /// {{ "foobar" is endingwith "bar" }} -> true
    /// {{ "foobar" is endingwith "foo" }} -> false
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn is_endingwith(v: Cow<'_, str>, other: Cow<'_, str>) -> bool {
        v.ends_with(&other as &str)
    }

    /// Test version of `==`.
    ///
    /// This is useful when combined with [`select`](crate::filters::select).
    ///
    /// ```jinja
    /// {{ 1 is eq 1 }} -> true
    /// {{ [1, 2, 3]|select("==", 1) }} => [1]
    /// ```
    ///
    /// By default aliased to `equalto` and `==`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    #[cfg(feature = "builtins")]
    pub fn is_eq(value: &Value, other: &Value) -> bool {
        *value == *other
    }

    /// Test version of `!=`.
    ///
    /// This is useful when combined with [`select`](crate::filters::select).
    ///
    /// ```jinja
    /// {{ 2 is ne 1 }} -> true
    /// {{ [1, 2, 3]|select("!=", 1) }} => [2, 3]
    /// ```
    ///
    /// By default aliased to `!=`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    #[cfg(feature = "builtins")]
    pub fn is_ne(value: &Value, other: &Value) -> bool {
        *value != *other
    }

    /// Test version of `<`.
    ///
    /// This is useful when combined with [`select`](crate::filters::select).
    ///
    /// ```jinja
    /// {{ 1 is lt 2 }} -> true
    /// {{ [1, 2, 3]|select("<", 2) }} => [1]
    /// ```
    ///
    /// By default aliased to `lessthan` and `<`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    #[cfg(feature = "builtins")]
    pub fn is_lt(value: &Value, other: &Value) -> bool {
        *value < *other
    }

    /// Test version of `<=`.
    ///
    /// This is useful when combined with [`select`](crate::filters::select).
    ///
    /// ```jinja
    /// {{ 1 is le 2 }} -> true
    /// {{ [1, 2, 3]|select("<=", 2) }} => [1, 2]
    /// ```
    ///
    /// By default aliased to `<=`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    #[cfg(feature = "builtins")]
    pub fn is_le(value: &Value, other: &Value) -> bool {
        *value <= *other
    }

    /// Test version of `>`.
    ///
    /// This is useful when combined with [`select`](crate::filters::select).
    ///
    /// ```jinja
    /// {{ 2 is gt 1 }} -> true
    /// {{ [1, 2, 3]|select(">", 2) }} => [3]
    /// ```
    ///
    /// By default aliased to `greaterthan` and `>`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    #[cfg(feature = "builtins")]
    pub fn is_gt(value: &Value, other: &Value) -> bool {
        *value > *other
    }

    /// Test version of `>=`.
    ///
    /// This is useful when combined with [`select`](crate::filters::select).
    ///
    /// ```jinja
    /// {{ 2 is ge 1 }} -> true
    /// {{ [1, 2, 3]|select(">=", 2) }} => [2, 3]
    /// ```
    ///
    /// By default aliased to `>=`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    #[cfg(feature = "builtins")]
    pub fn is_ge(value: &Value, other: &Value) -> bool {
        *value >= *other
    }

    /// Test version of `in`.
    ///
    /// ```jinja
    /// {{ 1 is in [1, 2, 3] }} -> true
    /// {{ [1, 2, 3]|select("in", [1, 2]) }} => [1, 2]
    /// ```
    ///
    /// This is useful when combined with [`select`](crate::filters::select).
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    #[cfg(feature = "builtins")]
    pub fn is_in(value: &Value, other: &Value) -> bool {
        crate::value::ops::contains(other, value)
            .map(|value| value.is_true())
            .unwrap_or(false)
    }

    #[test]
    fn test_basics() {
        fn test(_: &State, a: u32, b: u32) -> bool {
            assert_eq!(a, 23);
            a == b
        }

        let mut env = crate::Environment::new();
        env.add_test("test", test);
        assert!(crate::testutils::perform_test(
            &env,
            "test",
            &[Value::from(23), Value::from(23)][..]
        )
        .unwrap());
    }
}

#[cfg(feature = "builtins")]
pub use self::builtins::*;
