//! Utilities for unit testing.
//!
//! MiniJinja intentionally hides some unstable internal API.  However this hiding can be a
//! hindrance when doing unittests.  For this reason this module optionally exposes some
//! common utilities to test filters, tests, formatters and global functions.
//!
//! All these functions generally assume that the filter, test or else was first registered
//! on the environment before it's invoked.  While some filters can be invoked without being
//! registered first, it's generally advised against as filters can resolve other filters.
//! For instance the `select` filter and others will themselves discover further filters by
//! resolving them from the environment.
//!
//! ```
//! use minijinja::{Environment, Error};
//! use minijinja::testutils::apply_filter;
//!
//! fn add(a: u32, b: u32) -> Result<u32, Error> {
//!     Ok(a + b)
//! }
//!
//! let mut env = Environment::new();
//! env.add_filter("add", add);
//! assert_eq!(
//!     apply_filter(&env, "add", &[23.into(), 42.into()]).unwrap(),
//!     65.into()
//! );
//! ```

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::output::Output;
use crate::value::Value;
use crate::State;

/// Formats a value to a string using the formatter on the environment.
///
/// This function can be used to test custom formatters as some of the API that formatters
/// use is otherwise private.
///
/// ```
/// # use minijinja::{Environment, testutils::format};
/// let mut env = Environment::new();
/// env.set_formatter(|out, state, value| {
///     write!(out, "{:?}", value)?;
///     Ok(())
/// });
/// let rv = format(&env, "Hello World".into()).unwrap();
/// assert_eq!(rv, "\"Hello World\"");
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "testutils")))]
pub fn format(env: &Environment, value: Value) -> Result<String, Error> {
    let mut rv = String::new();
    let mut out = Output::with_string(&mut rv);
    State::with_dummy(env, |state| env.format(&value, state, &mut out)).map(|_| rv)
}

/// Invokes a filter of an environment.
///
/// ```
/// # use minijinja::{Environment, testutils::apply_filter};
/// let mut env = Environment::new();
/// let rv = apply_filter(&env, "upper", &["hello world".into()]).unwrap();
/// assert_eq!(rv.as_str(), Some("HELLO WORLD"));
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "testutils")))]
pub fn apply_filter(env: &Environment, filter: &str, args: &[Value]) -> Result<Value, Error> {
    State::with_dummy(env, |state| match env.get_filter(filter) {
        Some(filter) => filter.apply_to(state, args),
        None => Err(Error::from(ErrorKind::UnknownFilter)),
    })
}

/// Invokes a test of an environment.
///
/// ```
/// # use minijinja::{Environment, testutils::perform_test};
/// let mut env = Environment::new();
/// let rv = perform_test(&env, "even", &[42i32.into()]).unwrap();
/// assert!(rv);
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "testutils")))]
pub fn perform_test(env: &Environment, test: &str, args: &[Value]) -> Result<bool, Error> {
    State::with_dummy(env, |state| match env.get_test(test) {
        Some(test) => test.perform(state, args),
        None => Err(Error::from(ErrorKind::UnknownTest)),
    })
}

/// Invokes a global function.
///
/// ```
/// # use minijinja::{Environment, testutils::invoke_global};
/// let mut env = Environment::new();
/// let rv = invoke_global(&env, "range", &[3u32.into()]).unwrap();
/// assert_eq!(rv.to_string(), "[0, 1, 2]");
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "testutils")))]
pub fn invoke_global(env: &Environment, func: &str, args: &[Value]) -> Result<Value, Error> {
    State::with_dummy(env, |state| match env.globals.get(func) {
        Some(func) => func.call(state, args),
        None => Err(Error::from(ErrorKind::UnknownFunction)),
    })
}
