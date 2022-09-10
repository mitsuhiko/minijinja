//! Global functions and abstractions.
//!
//! This module provides the abstractions for functions that can registered as
//! global functions to the environment via
//! [`add_function`](crate::Environment::add_function).
//!
//! # Using Functions
//!
//! Functions can be called in any place where an expression is valid.  They
//! are useful to retrieve data.  Some functions are special and provided
//! by the engine (like `super`) within certain context, others are global.
//!
//! The following is a motivating example:
//!
//! ```jinja
//! <pre>{{ debug() }}</pre>
//! ```
//!
//! # Custom Functions
//!
//! A custom global function is just a simple rust function which accepts the
//! [`&State`](crate::State) as first argument, optionally some additional
//! arguments and then returns a result.  Global functions are typically used to
//! perform a data loading operation.  For instance these functions can be used
//! to expose data to the template that hasn't been provided by the individual
//! render invocation.
//!
//! ```rust
//! # use minijinja::Environment;
//! # let mut env = Environment::new();
//! use minijinja::{State, Error, ErrorKind};
//!
//! fn include_file(_state: &State, name: String) -> Result<String, Error> {
//!     std::fs::read_to_string(&name)
//!         .map_err(|e| Error::new(
//!             ErrorKind::ImpossibleOperation,
//!             "cannot load file"
//!         ).with_source(e))
//! }
//!
//! env.add_function("include_file", include_file);
//! ```
//!
//! # Built-in Functions
//!
//! When the `builtins` feature is enabled a range of built-in functions are
//! automatically added to the environment.  These are also all provided in
//! this module.  Note though that these functions are not to be
//! called from Rust code as their exact interface (arguments and return types)
//! might change from one MiniJinja version to another.
use std::fmt;
use std::sync::Arc;

use crate::error::Error;
use crate::utils::SealedMarker;
use crate::value::{FunctionArgs, FunctionResult, Object, Value};
use crate::vm::State;

type FuncFunc = dyn Fn(&State, &[Value]) -> Result<Value, Error> + Sync + Send + 'static;

/// A boxed function.
#[derive(Clone)]
pub(crate) struct BoxedFunction(Arc<FuncFunc>, &'static str);

/// A utility trait that represents global functions.
///
/// This trait is used by the [`add_functino`](crate::Environment::add_function)
/// method to abstract over different types of functions.
///
/// Functions which at the very least accept the [`State`] by reference as first
/// parameter and additionally up to 4 further parameters.  They share much of
/// their interface with [`filters`](crate::filters).
///
/// A function can return any of the following types:
///
/// * `Rv` where `Rv` implements `Into<Value>`
/// * `Result<Rv, Error>` where `Rv` implements `Into<Value>`
///
/// The parameters can be marked optional by using `Option<T>`.  The last
/// argument can also use [`Rest<T>`](crate::value::Rest) to capture the
/// remaining arguments.  All types are supported for which
/// [`ArgType`](crate::value::ArgType) is implemented.
///
/// For a list of built-in functions see [`functions`](crate::functions).
///
/// # Basic Example
///
/// ```rust
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::{State, Error, ErrorKind};
///
/// fn include_file(_state: &State, name: String) -> Result<String, Error> {
///     std::fs::read_to_string(&name)
///         .map_err(|e| Error::new(
///             ErrorKind::ImpossibleOperation,
///             "cannot load file"
///         ).with_source(e))
/// }
///
/// env.add_function("include_file", include_file);
/// ```
///
/// ```jinja
/// {{ include_file("filname.txt") }}
/// ```
///
/// # Variadic
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::State;
/// use minijinja::value::Rest;
///
/// fn sum(_state: &State, values: Rest<i64>) -> i64 {
///     values.iter().sum()
/// }
///
/// env.add_function("sum", sum);
/// ```
///
/// ```jinja
/// {{ sum(1, 2, 3) }} -> 6
/// ```
pub trait Function<Rv, Args>: Send + Sync + 'static {
    /// Calls a function with the given arguments.
    #[doc(hidden)]
    fn invoke(&self, env: &State, args: Args, _: SealedMarker) -> Rv;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, Rv, $($name),*> Function<Rv, ($($name,)*)> for Func
        where
            Func: Fn(&State, $($name),*) -> Rv + Send + Sync + 'static,
            Rv: FunctionResult,
        {
            fn invoke(&self, state: &State, args: ($($name,)*), _: SealedMarker) -> Rv {
                #[allow(non_snake_case)]
                let ($($name,)*) = args;
                (self)(state, $($name,)*)
            }
        }
    };
}

tuple_impls! {}
tuple_impls! { A }
tuple_impls! { A B }
tuple_impls! { A B C }
tuple_impls! { A B C D }

impl BoxedFunction {
    /// Creates a new boxed filter.
    pub fn new<F, Rv, Args>(f: F) -> BoxedFunction
    where
        F: Function<Rv, Args> + for<'a> Function<Rv, <Args as FunctionArgs<'a>>::Output>,
        Rv: FunctionResult,
        Args: for<'a> FunctionArgs<'a>,
    {
        BoxedFunction(
            Arc::new(move |state, args| -> Result<Value, Error> {
                f.invoke(state, Args::from_values(args)?, SealedMarker)
                    .into_result()
            }),
            std::any::type_name::<F>(),
        )
    }

    /// Invokes the function.
    pub fn invoke(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        (self.0)(state, args)
    }

    /// Creates a value from a boxed function.
    pub fn to_value(&self) -> Value {
        Value::from_object(self.clone())
    }
}

impl fmt::Debug for BoxedFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            if self.1.is_empty() {
                "BoxedFunction"
            } else {
                self.1
            }
        )
    }
}

impl fmt::Display for BoxedFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Object for BoxedFunction {
    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        self.invoke(state, args)
    }
}

#[cfg(feature = "builtins")]
mod builtins {
    use super::*;

    use std::collections::BTreeMap;

    use crate::error::ErrorKind;
    use crate::value::ValueKind;

    /// Returns a range.
    ///
    /// Return a list containing an arithmetic progression of integers. `range(i,
    /// j)` returns `[i, i+1, i+2, ..., j-1]`. `lower` defaults to 0. When `step` is
    /// given, it specifies the increment (or decrement). For example, `range(4)`
    /// and `range(0, 4, 1)` return `[0, 1, 2, 3]`. The end point is omitted.
    ///
    /// ```jinja
    /// <ul>
    /// {% for num in range(1, 11) %}
    ///   <li>{{ num }}
    /// {% endfor %}
    /// </ul>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn range(_state: &State, lower: u32, upper: Option<u32>, step: Option<u32>) -> Vec<u32> {
        let rng = match upper {
            Some(upper) => lower..upper,
            None => 0..lower,
        };
        if let Some(step) = step {
            rng.step_by(step as usize).collect()
        } else {
            rng.collect()
        }
    }

    /// Creates a dictionary.
    ///
    /// This is a convenient alternative for a dictionary literal.
    /// `{"foo": "bar"}` is the same as `dict(foo="bar")`.
    ///
    /// ```jinja
    /// <script>const CONFIG = {{ dict(
    ///   DEBUG=true,
    ///   API_URL_PREFIX="/api"
    /// )|tojson }};</script>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn dict(_state: &State, value: Value) -> Result<Value, Error> {
        if value.is_undefined() {
            Ok(Value::from(BTreeMap::<bool, Value>::new()))
        } else if value.kind() != ValueKind::Map {
            Err(Error::from(ErrorKind::ImpossibleOperation))
        } else {
            Ok(value)
        }
    }

    /// Outputs the current context stringified.
    ///
    /// This is a useful function to quickly figure out the state of affairs
    /// in a template.  It emits a stringified debug dump of the current
    /// engine state including the layers of the context, the current block
    /// and auto escaping setting.
    ///
    /// ```jinja
    /// <pre>{{ debug() }}</pre>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn debug(state: &State) -> String {
        format!("{:#?}", state)
    }
}

#[cfg(feature = "builtins")]
pub use self::builtins::*;
