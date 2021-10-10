//! Global functions and abstractions.
//!
//! This module provides the abstractions for functions that can registered as
//! global functions to the environment via
//! [`add_function`](crate::Environment::add_function).
//!
//! # Custom Functions
//!
//! A custom global function is just a simple rust function which accepts the
//! environment as first argument, optionally some additional arguments and then
//! returns a result.  Global functions are typically used to perform a data
//! loading operation.  For instance these functions can be used to expose data
//! to the template that hasn't been provided by the individual render invocation.
//!
//! ```rust
//! # use minijinja::{Environment, Error, ErrorKind};
//! # let mut env = Environment::new();
//! fn include_file(env: &Environment, name: String) -> Result<String, Error> {
//!     std::fs::read_to_string(&name)
//!         .map_err(|e| Error::new(
//!             ErrorKind::ImpossibleOperation,
//!             "cannot load file"
//!         ).with_source(e))
//! }
//!
//! env.add_function("include_file", include_file);
//! ```
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::environment::Environment;
use crate::error::Error;
use crate::value::{FunctionArgs, Object, Value};

type FuncFunc = dyn Fn(&Environment, Vec<Value>) -> Result<Value, Error> + Sync + Send + 'static;

/// A boxed function.
#[derive(Clone)]
pub(crate) struct BoxedFunction(Arc<FuncFunc>);

/// A utility trait that represents global functions.
pub trait Function<Rv = Value, Args = Vec<Value>>: Send + Sync + 'static {
    /// Calls a functionw ith the given arguments.
    fn invoke(&self, env: &Environment, args: Args) -> Result<Rv, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<F, Rv, $($name),*> Function<Rv, ($($name,)*)> for F
        where
            F: Fn(&Environment, $($name),*) -> Result<Rv, Error> + Send + Sync + 'static
        {
            fn invoke(&self, env: &Environment, args: ($($name,)*)) -> Result<Rv, Error> {
                #[allow(non_snake_case)]
                let ($($name,)*) = args;
                (self)(env, $($name,)*)
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
        F: Function<Rv, Args>,
        Rv: Into<Value>,
        Args: FunctionArgs,
    {
        BoxedFunction(Arc::new(move |env, args| -> Result<Value, Error> {
            f.invoke(env, FunctionArgs::from_values(args)?)
                .map(Into::into)
        }))
    }

    /// Applies the filter to a value and argument.
    pub fn invoke(&self, env: &Environment, args: Vec<Value>) -> Result<Value, Error> {
        (self.0)(env, args)
    }

    /// Creates a value from a boxed function.
    pub fn to_value(&self) -> Value {
        Value::from_object(self.clone())
    }
}

impl fmt::Debug for BoxedFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedFunc").finish()
    }
}

impl fmt::Display for BoxedFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedFunc").finish()
    }
}

impl Object for BoxedFunction {
    fn call(&self, env: &Environment, args: Vec<Value>) -> Result<Value, Error> {
        self.invoke(env, args)
    }
}

pub(crate) fn get_globals() -> BTreeMap<&'static str, Value> {
    #[allow(unused_mut)]
    let mut rv = BTreeMap::new();
    #[cfg(feature = "builtin_functions")]
    {
        rv.insert("range", BoxedFunction::new(range).to_value());
        rv.insert("dict", BoxedFunction::new(dict).to_value());
    }
    rv
}

#[cfg(feature = "builtin_functions")]
mod builtins {
    use super::*;

    use crate::error::ErrorKind;
    use crate::value::ValueKind;

    /// Returns a range.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_functions")))]
    pub fn range(
        _env: &Environment,
        lower: u32,
        upper: Option<u32>,
        step: Option<u32>,
    ) -> Result<Vec<u32>, Error> {
        let rng = match upper {
            Some(upper) => (lower..upper),
            None => (0..lower),
        };
        Ok(if let Some(step) = step {
            rng.step_by(step as usize).collect()
        } else {
            rng.collect()
        })
    }

    /// Creates a dictionary.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_functions")))]
    pub fn dict(_env: &Environment, value: Value) -> Result<Value, Error> {
        if value.is_undefined() {
            Ok(Value::from(BTreeMap::<bool, Value>::new()))
        } else if value.kind() != ValueKind::Map {
            Err(Error::from(ErrorKind::ImpossibleOperation))
        } else {
            Ok(value)
        }
    }
}

#[cfg(feature = "builtin_functions")]
pub use self::builtins::*;
