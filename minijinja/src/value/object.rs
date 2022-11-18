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
///
/// The exact runtime characteristics of the object are influenced by the
/// [`kind`](Self::kind) of the object.  By default an object can just be
/// stringified and methods can be called.
///
/// For examples of how to implement objects refer to [`SeqObject`] and
/// [`StructObject`].
pub trait Object: fmt::Display + fmt::Debug + Any + Sync + Send {
    /// Describes the kind of an object.
    ///
    /// If not implemented behavior for an object is [`ObjectKind::Basic`]
    /// which just means that it's stringifyable and potentially can be
    /// called or has methods.
    ///
    /// For more information see [`ObjectKind`].
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Basic
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
            ErrorKind::UnknownMethod,
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
            ErrorKind::InvalidOperation,
            "tried to call non callable object",
        ))
    }
}

impl<T: Object> Object for std::sync::Arc<T> {
    fn kind(&self) -> ObjectKind<'_> {
        T::kind(self)
    }

    fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        T::call_method(self, state, name, args)
    }

    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        T::call(self, state, args)
    }
}

/// A kind defines the object's behavior.
///
/// When a dynamic [`Object`] is implemented, it can be of one of the kinds
/// here.  The default behavior will be a [`Basic`](Self::Basic) object which
/// doesn't do much other than that it can be printed.  For an object to turn
/// into a [struct](Self::Struct) or [sequence](Self::Seq) the necessary kind
/// has to be returned with a pointer to itself.
///
/// Today object's can have the behavior of structs and sequences but this
/// might expand in the future.  It does mean that not all types of values can
/// be represented by objects.
#[non_exhaustive]
pub enum ObjectKind<'a> {
    /// This object is a basic object.
    ///
    /// Such an object has no attributes but it might be callable and it
    /// can be stringified.  When serialized it's serialized in it's
    /// stringified form.
    Basic,

    /// This object is a sequence.
    ///
    /// Requires that the object implements [`SeqObject`].
    Seq(&'a dyn SeqObject),

    /// This object is a struct (map with string keys).
    ///
    /// Requires that the object implements [`StructObject`].
    Struct(&'a dyn StructObject),
}

/// Views an [`Object`] as sequence of values.
///
/// # Example
///
/// The following example shows how to implement a dynamic object which
/// represents a sequence of three items:
///
/// ```
/// use std::fmt;
/// use minijinja::value::{Value, Object, ObjectKind, SeqObject};
///
/// #[derive(Debug, Clone)]
/// struct Point(f32, f32, f32);
///
/// impl fmt::Display for Point {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "({}, {}, {})", self.0, self.1, self.2)
///     }
/// }
///
/// impl Object for Point {
///     fn kind(&self) -> ObjectKind<'_> {
///         ObjectKind::Seq(self)
///     }
/// }
///
/// impl SeqObject for Point {
///     fn get(&self, idx: usize) -> Option<Value> {
///         match idx {
///             0 => Some(Value::from(self.0)),
///             1 => Some(Value::from(self.1)),
///             2 => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn len(&self) -> usize {
///         3
///     }
/// }
///
/// let value = Value::from_object(Point(1.0, 2.5, 3.0));
/// ```
pub trait SeqObject {
    /// Looks up an item by index.
    fn get(&self, idx: usize) -> Option<Value>;

    /// Returns the number of items in the sequence.
    fn len(&self) -> usize;

    /// Checks if the struct is empty.
    ///
    /// The default implementation checks if the length is 0.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Views an [`Object`] as a struct.
///
/// # Example
///
/// The following example shows how to implement a dynamic object which
/// represents a struct:
///
/// ```
/// use std::fmt;
/// use minijinja::value::{Value, Object, ObjectKind, StructObject};
///
/// #[derive(Debug, Clone)]
/// struct Point(f32, f32, f32);
///
/// impl fmt::Display for Point {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "({}, {}, {})", self.0, self.1, self.2)
///     }
/// }
///
/// impl Object for Point {
///     fn kind(&self) -> ObjectKind<'_> {
///         ObjectKind::Struct(self)
///     }
/// }
///
/// impl StructObject for Point {
///     fn get(&self, name: &str) -> Option<Value> {
///         match name {
///             "x" => Some(Value::from(self.0)),
///             "y" => Some(Value::from(self.1)),
///             "z" => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn fields(&self) -> Box<dyn Iterator<Item = &str> + '_> {
///         Box::new(["x", "y", "z"].into_iter())
///     }
/// }
///
/// let value = Value::from_object(Point(1.0, 2.5, 3.0));
/// ```
pub trait StructObject {
    /// Invoked by the engine to get a field of a struct.
    ///
    /// Where possible it's a good idea for this to align with the return value
    /// of [`fields`](Self::fields) but it's not necessary.
    ///
    /// If an field does not exist, `None` shall be returned.
    ///
    /// A note should be made here on side effects: unlike calling objects or
    /// calling methods on objects, accessing fields is not supposed to
    /// have side effects.  Neither does this API get access to the interpreter
    /// [`State`] nor is there a channel to send out failures as only an option
    /// can be returned.  If you do plan on doing something in field access
    /// that is fallible, instead use a method call.
    fn get(&self, idx: &str) -> Option<Value>;

    /// Iterates over the fields.
    ///
    /// The default implementation returns an empty iterator.
    fn fields(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(None.into_iter())
    }

    /// Returns the number of fields in the struct.
    ///
    /// The default implementation returns the number of fields.
    fn len(&self) -> usize {
        self.fields().count()
    }

    /// Checks if the struct is empty.
    ///
    /// The default implementation checks if the length is 0.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
