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
//! # use minijinja::{Environment, State, Error, ErrorKind};
//! # let mut env = Environment::new();
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
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::error::Error;
use crate::value::{FunctionArgs, Object, Value};
use crate::vm::State;

type FuncFunc = dyn Fn(&State, Vec<Value>) -> Result<Value, Error> + Sync + Send + 'static;

/// A boxed function.
#[derive(Clone)]
pub(crate) struct BoxedFunction(Arc<FuncFunc>, &'static str);

/// A utility trait that represents global functions.
pub trait Function<Rv = Value, Args = Vec<Value>>: Send + Sync + 'static {
    /// Calls a function with the given arguments.
    fn invoke(&self, env: &State, args: Args) -> Result<Rv, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<F, Rv, $($name),*> Function<Rv, ($($name,)*)> for F
        where
            F: Fn(&State, $($name),*) -> Result<Rv, Error> + Send + Sync + 'static
        {
            fn invoke(&self, state: &State, args: ($($name,)*)) -> Result<Rv, Error> {
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
        F: Function<Rv, Args>,
        Rv: Into<Value>,
        Args: FunctionArgs,
    {
        BoxedFunction(
            Arc::new(move |env, args| -> Result<Value, Error> {
                f.invoke(env, FunctionArgs::from_values(args)?)
                    .map(Into::into)
            }),
            std::any::type_name::<F>(),
        )
    }

    /// Invokes the function.
    pub fn invoke(&self, state: &State, args: Vec<Value>) -> Result<Value, Error> {
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
    fn call(&self, state: &State, args: Vec<Value>) -> Result<Value, Error> {
        self.invoke(state, args)
    }
}

pub(crate) fn get_globals() -> BTreeMap<&'static str, Value> {
    #[allow(unused_mut)]
    let mut rv = BTreeMap::new();
    #[cfg(feature = "builtin_functions")]
    {
        rv.insert("range", BoxedFunction::new(range).to_value());
        rv.insert("dict", BoxedFunction::new(dict).to_value());
        rv.insert("debug", BoxedFunction::new(debug).to_value());
    }
    rv
}

#[cfg(feature = "builtin_functions")]
mod builtins {
    use super::*;

    use crate::error::ErrorKind;
    use crate::value::ValueKind;

    /// Returns a range.
    ///
    /// Return a list containing an arithmetic progression of integers. `range(i,
    /// j)` returns `[i, i+1, i+2, ..., j-1]`. `lower` defaults to 0. When `step` is
    /// given, it specifies the increment (or decrement). For example, `range(4)`
    /// and `range(0, 4, 1)` return `[0, 1, 2, 3]`. The end point is omitted.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_functions")))]
    pub fn range(
        _state: &State,
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
    ///
    /// This is a convenient alternative for a dictionary literal.
    /// `{"foo": "bar"}` is the same as `dict(foo="bar")`.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_functions")))]
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
    #[cfg_attr(docsrs, doc(cfg(feature = "builtin_functions")))]
    pub fn debug(state: &State) -> Result<String, Error> {
        Ok(format!("{:#?}", state))
    }
}

#[cfg(feature = "builtin_functions")]
pub use self::builtins::*;
