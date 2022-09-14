use std::any::Any;
use std::fmt;

use crate::error::{Error, ErrorKind};
use crate::value::Value;
use crate::vm::State;

/// A utility trait that represents a dynamic object.
///
/// The engine uses the [`Value`] type to represent values that the engine
/// knows about.  Most of these values are primitives such as integers, strings
/// or maps.  However it is also possible to expose custom types without
/// undergoing a serialization step to the engine.  For this to work a type
/// needs to implement the [`Object`] trait and be wrapped in a value with
/// [`Value::from_object`](crate::value::Value::from_object). The ownership of
/// the object will then move into the value type.
//
/// The engine uses reference counted objects with interior mutability in the
/// value type.  This means that all trait methods take `&self` and types like
/// [`Mutex`](std::sync::Mutex) need to be used to enable mutability.
//
/// Objects need to implement [`Display`](std::fmt::Display) which is used by
/// the engine to convert the object into a string if needed.  Additionally
/// [`Debug`](std::fmt::Debug) is required as well.
pub trait Object: fmt::Display + fmt::Debug + Any + Sync + Send {
    /// Invoked by the engine to get the attribute of an object.
    ///
    /// Where possible it's a good idea for this to align with the return value
    /// of [`attributes`](Self::attributes) but it's not necessary.
    ///
    /// If an attribute does not exist, `None` shall be returned.
    fn get_attr(&self, name: &str) -> Option<Value> {
        let _name = name;
        None
    }

    /// An enumeration of attributes that are known to exist on this object.
    ///
    /// The default implementation returns an empty slice.  If it's not possible
    /// to implement this, it's fine for the implementation to be omitted.  The
    /// enumeration here is used by the `for` loop to iterate over the attributes
    /// on the value.
    fn attributes(&self) -> &[&str] {
        &[][..]
    }

    /// Called when the engine tries to call a method on the object.
    ///
    /// It's the responsibility of the implementer to ensure that an
    /// error is generated if an invalid method is invoked.
    ///
    /// To convert the arguments into arguments use the
    /// [`from_args`](crate::value::from_args) function.
    fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        let _state = state;
        let _args = args;
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            format!("object has no method named {}", name),
        ))
    }

    /// Called when the object is invoked directly.
    ///
    /// The default implementation just generates an error that the object
    /// cannot be invoked.
    ///
    /// To convert the arguments into arguments use the
    /// [`from_args`](crate::value::from_args) function.
    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        let _state = state;
        let _args = args;
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            "tried to call non callable object",
        ))
    }
}
