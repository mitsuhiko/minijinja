use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use crate::error::{Error, ErrorKind, Result};
use crate::value::{intern, Value, ValueMap, ValueRepr};
use crate::vm::State;

/// A trait that represents a dynamic object.
///
/// There is a type erased wrapper of this trait available called
/// [`DynObject`] which is what the engine actually holds internally.
///
/// # Basic Struct
///
/// The following example shows how to implement a dynamic object which
/// represents a struct.  All that's needed is to implement
/// [`get_value`](Self::get_value) to look up a field by name as well as
/// [`enumerate`](Self::enumerate) to return an enumerator over the known keys.
/// The [`repr`](Self::repr) defaults to `Map` so nothing needs to be done here.
///
/// ```
/// use std::sync::Arc;
/// use minijinja::value::{Value, Object, Enumerator};
///
/// #[derive(Debug)]
/// struct Point(f32, f32, f32);
///
/// impl Object for Point {
///     fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
///         match key.as_str()? {
///             "x" => Some(Value::from(self.0)),
///             "y" => Some(Value::from(self.1)),
///             "z" => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn enumerate(self: &Arc<Self>) -> Enumerator {
///         Enumerator::Str(&["x", "y", "z"])
///     }
/// }
///
/// let value = Value::from_object(Point(1.0, 2.5, 3.0));
/// ```
///
/// # Basic Sequence
///
/// The following example shows how to implement a dynamic object which
/// represents a sequence.  All that's needed is to implement
/// [`repr`](Self::repr) to indicate that this is a sequence,
/// [`get_value`](Self::get_value) to look up a field by index, and
/// [`enumerate`](Self::enumerate) to return a sequential enumerator.
/// This enumerator will automatically call `get_value` from `0..length`.
///
/// ```
/// use std::sync::Arc;
/// use minijinja::value::{Value, Object, ObjectRepr, Enumerator};
///
/// #[derive(Debug)]
/// struct Point(f32, f32, f32);
///
/// impl Object for Point {
///     fn repr(self: &Arc<Self>) -> ObjectRepr {
///         ObjectRepr::Seq
///     }
///
///     fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
///         match key.as_usize()? {
///             0 => Some(Value::from(self.0)),
///             1 => Some(Value::from(self.1)),
///             2 => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn enumerate(self: &Arc<Self>) -> Enumerator {
///         Enumerator::Seq(3)
///     }
/// }
///
/// let value = Value::from_object(Point(1.0, 2.5, 3.0));
/// ```
///
/// # Iterables
///
/// If you have something that is not quite a sequence but is capable of yielding
/// values over time, you can directly implement an iterable.  This is somewhat
/// uncommon as you can normally directly use [`Value::make_iterable`].  Here
/// is how this can be done though:
///
/// ```
/// use std::sync::Arc;
/// use minijinja::value::{Value, Object, ObjectRepr, Enumerator};
///
/// #[derive(Debug)]
/// struct Range10;
///
/// impl Object for Range10 {
///     fn repr(self: &Arc<Self>) -> ObjectRepr {
///         ObjectRepr::Iterable
///     }
///
///     fn enumerate(self: &Arc<Self>) -> Enumerator {
///         Enumerator::Iter(Box::new((1..10).map(Value::from)))
///     }
/// }
///
/// let value = Value::from_object(Range10);
/// ```
///
/// # Map As Context
///
/// Map can also be used as template rendering context.  This has a lot of
/// benefits as it means that the serialization overhead can be largely to
/// completely avoided.  This means that even if templates take hundreds of
/// values, MiniJinja does not spend time eagerly converting them into values.
///
/// Here is a very basic example of how a template can be rendered with a dynamic
/// context.  Note that the implementation of [`enumerate`](Self::enumerate)
/// is optional for this to work.  It's in fact not used by the engine during
/// rendering but it is necessary for the [`debug()`](crate::functions::debug)
/// function to be able to show which values exist in the context.
///
/// ```
/// # fn main() -> Result<(), minijinja::Error> {
/// # use minijinja::Environment;
/// use std::sync::Arc;
/// use minijinja::value::{Value, Object};
///
/// #[derive(Debug)]
/// pub struct DynamicContext {
///     magic: i32,
/// }
///
/// impl Object for DynamicContext {
///     fn get_value(self: &Arc<Self>, field: &Value) -> Option<Value> {
///         match field.as_str()? {
///             "pid" => Some(Value::from(std::process::id())),
///             "env" => Some(Value::from_iter(std::env::vars())),
///             "magic" => Some(Value::from(self.magic)),
///             _ => None,
///         }
///     }
/// }
///
/// # let env = Environment::new();
/// let tmpl = env.template_from_str("HOME={{ env.HOME }}; PID={{ pid }}; MAGIC={{ magic }}")?;
/// let ctx = Value::from_object(DynamicContext { magic: 42 });
/// let rv = tmpl.render(ctx)?;
/// # Ok(()) }
/// ```
///
/// One thing of note here is that in the above example `env` would be re-created every
/// time the template needs it.  A better implementation would cache the value after it
/// was created first.
pub trait Object: fmt::Debug + Send + Sync {
    /// Indicates the natural representation of an object.
    ///
    /// The default implementation returns [`ObjectRepr::Map`].
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    /// Given a key, looks up the associated value.
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let _ = key;
        None
    }

    /// Enumerates the object.
    ///
    /// For more information see [`Enumerator`].  The default implementation
    /// returns an `Empty` enumerator if the object repr is a `Map` or `Seq`,
    /// and `NonEnumerable` for `Plain` objects or `Iterator`s.
    fn enumerate(self: &Arc<Self>) -> Enumerator {
        match self.repr() {
            ObjectRepr::Plain | ObjectRepr::Iterable => Enumerator::NonEnumerable,
            ObjectRepr::Map | ObjectRepr::Seq => Enumerator::Empty,
        }
    }

    /// Returns the length of the object.
    ///
    /// By default the length is taken from [`Enumerator::len`].  This means that in order
    /// to determine the length, an iteration is started.  If you this is a problem for
    /// your uses, you can manually implement this.  This might for instance be needed
    /// if your type can only be iterated over once.
    fn len(self: &Arc<Self>) -> Option<usize> {
        self.enumerate().len()
    }

    /// Returns `true` if this object is considered empty.
    ///
    /// The default implementation checks if the length of the object is `Some(0)` which
    /// is the recommended behavior for objects.
    fn is_empty(self: &Arc<Self>) -> bool {
        self.len() == Some(0)
    }

    /// The engine calls this to invoke the object itself.
    ///
    /// The default implementation returns an
    /// [`InvalidOperation`](crate::ErrorKind::InvalidOperation) error.
    fn call(self: &Arc<Self>, state: &State<'_, '_>, args: &[Value]) -> Result<Value> {
        let (_, _) = (state, args);
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "object is not callable",
        ))
    }

    /// The engine calls this to invoke a method on the object.
    ///
    /// The default implementation returns an
    /// [`UnknownMethod`](crate::ErrorKind::UnknownMethod) error.  When this error
    /// is returned the engine will invoke the
    /// [`unknown_method_callback`](crate::Environment::set_unknown_method_callback) of
    /// the environment.
    fn call_method(
        self: &Arc<Self>,
        state: &State<'_, '_>,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        if let Some(value) = self.get_value(&Value::from(method)) {
            return value.call(state, args);
        }

        Err(Error::new(
            ErrorKind::UnknownMethod,
            "object has no such method",
        ))
    }

    /// Formats the object for stringification.
    ///
    /// The default implementation is specific to the behavior of
    /// [`repr`](Self::repr) and usually does not need modification.
    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result
    where
        Self: Sized + 'static,
    {
        struct Dbg<'a>(pub &'a Value);

        impl<'a> fmt::Debug for Dbg<'a> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if let ValueRepr::Object(ref obj) = self.0 .0 {
                    obj.render(f)
                } else {
                    fmt::Debug::fmt(&self.0, f)
                }
            }
        }

        match self.repr() {
            ObjectRepr::Map => {
                let mut dbg = f.debug_map();
                for (key, value) in self.try_iter_pairs().into_iter().flatten() {
                    dbg.entry(&Dbg(&key), &Dbg(&value));
                }
                dbg.finish()
            }
            // for either sequences or iterables, a length is needed, otherwise we
            // don't want to risk iteration during printing and fall back to the
            // debug print.
            ObjectRepr::Seq | ObjectRepr::Iterable if self.len().is_some() => {
                let mut dbg = f.debug_list();
                for value in self.try_iter().into_iter().flatten() {
                    dbg.entry(&Dbg(&value));
                }
                dbg.finish()
            }
            _ => {
                write!(f, "{self:?}")
            }
        }
    }
}

macro_rules! impl_object_helpers {
    ($vis:vis $self_ty: ty) => {
        /// Iterates over this object.
        ///
        /// If this returns `None` then the default object iteration as defined by
        /// the object's `enumeration` is used.
        $vis fn try_iter(self: $self_ty) -> Option<Box<dyn Iterator<Item = Value> + Send + Sync>>
        where
            Self: 'static,
        {
            match self.enumerate() {
                Enumerator::NonEnumerable => None,
                Enumerator::Empty => Some(Box::new(None::<Value>.into_iter())),
                Enumerator::Seq(l) => {
                    let self_clone = self.clone();
                    Some(Box::new((0..l).map(move |idx| {
                        self_clone.get_value(&Value::from(idx)).unwrap_or_default()
                    })))
                }
                Enumerator::Iter(iter) => Some(iter),
                Enumerator::RevIter(iter) => Some(Box::new(iter)),
                Enumerator::Str(s) => Some(Box::new(s.iter().copied().map(intern).map(Value::from))),
                Enumerator::Values(v) => Some(Box::new(v.into_iter())),
            }
        }

        /// Iterate over key and value at once.
        $vis fn try_iter_pairs(
            self: $self_ty,
        ) -> Option<Box<dyn Iterator<Item = (Value, Value)> + Send + Sync>> {
            let iter = some!(self.try_iter());
            let repr = self.repr();
            let self_clone = self.clone();
            Some(Box::new(iter.enumerate().map(move |(idx, item)| {
                match repr {
                    ObjectRepr::Map => {
                        let value = self_clone.get_value(&item);
                        (item, value.unwrap_or_default())
                    }
                    _ => (Value::from(idx), item)
                }
            })))
        }
    };
}

/// Provides utility methods for working with objects.
pub trait ObjectExt: Object + Send + Sync + 'static {
    /// Creates a new enumeration that projects into the given object.
    fn mapped_enumerator<F>(self: &Arc<Self>, maker: F) -> Enumerator
    where
        F: for<'a> FnOnce(&'a Self) -> Box<dyn Iterator<Item = Value> + Send + Sync + 'a>
            + Send
            + Sync
            + 'static,
        Self: Sized,
    {
        struct IterObject<T> {
            iter: Box<dyn Iterator<Item = Value> + Send + Sync + 'static>,
            _object: Arc<T>,
        }

        impl<T> Iterator for IterObject<T> {
            type Item = Value;

            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next()
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
        }

        // SAFETY: this is safe because the `IterObject` will keep our object alive.
        let iter = unsafe { std::mem::transmute(maker(self)) };
        let _object = self.clone();
        Enumerator::Iter(Box::new(IterObject { iter, _object }))
    }

    /// Creates a new enumeration that projects into the given object supporting reversing.
    fn mapped_rev_enumerator<F>(self: &Arc<Self>, maker: F) -> Enumerator
    where
        F: for<'a> FnOnce(
                &'a Self,
            )
                -> Box<dyn DoubleEndedIterator<Item = Value> + Send + Sync + 'a>
            + Send
            + Sync
            + 'static,
        Self: Sized,
    {
        struct IterObject<T> {
            iter: Box<dyn DoubleEndedIterator<Item = Value> + Send + Sync + 'static>,
            _object: Arc<T>,
        }

        impl<T> Iterator for IterObject<T> {
            type Item = Value;

            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next()
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
        }

        impl<T> DoubleEndedIterator for IterObject<T> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.iter.next_back()
            }
        }

        // SAFETY: this is safe because the `IterObject` will keep our object alive.
        let iter = unsafe { std::mem::transmute(maker(self)) };
        let _object = self.clone();
        Enumerator::RevIter(Box::new(IterObject { iter, _object }))
    }

    impl_object_helpers!(&Arc<Self>);
}

impl<T: Object + Send + Sync + 'static> ObjectExt for T {}

/// Utility type to enumerate an object.
#[non_exhaustive]
pub enum Enumerator {
    /// A non enumerable enumeration.
    ///
    /// This fails iteration and the object has no known length.
    NonEnumerable,
    /// The empty enumeration.  It yields no elements.
    ///
    /// It has a known length of 0.
    Empty,
    /// A slice of static string keys.
    ///
    /// This has a known length which is the length of the slice.
    Str(&'static [&'static str]),
    /// A dynamic iterator over values.  Length is known if the size hint has matching lower and upper bounds.
    Iter(Box<dyn Iterator<Item = Value> + Send + Sync>),
    /// Like `Iter` but supports efficient reversing.
    RevIter(Box<dyn DoubleEndedIterator<Item = Value> + Send + Sync>),
    /// Instructs the engine to yield values by calling `get_value` from 0 to `usize`.
    ///
    /// This has a known legth of `usize`.
    Seq(usize),
    /// A vector of known values to iterate over.
    ///
    /// This has a known length which is the length of the vector.
    Values(Vec<Value>),
}

/// Defines the natural representation of this object.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ObjectRepr {
    /// An object that has no reasonable representation.  Usually stringifies.
    Plain,
    /// serializes to {...} and over the enumeration, values
    Map,
    /// serializes to [...] over its values
    Seq,
    /// Similar to `Seq` but without indexing
    Iterable,
}

type_erase! {
    pub trait Object: Send + Sync => DynObject(DynObjectVT) {
        fn repr(&self) -> ObjectRepr;

        fn get_value(&self, key: &Value) -> Option<Value>;

        fn enumerate(&self) -> Enumerator;

        fn is_empty(&self) -> bool;

        fn len(&self) -> Option<usize>;

        fn call(
            &self,
            state: &State<'_, '_>,
            args: &[Value]
        ) -> Result<Value>;

        fn call_method(
            &self,
            state: &State<'_, '_>,
            method: &str,
            args: &[Value]
        ) -> Result<Value>;

        fn render(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;

        impl fmt::Debug {
            fn fmt[debug](&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
        }
    }
}

impl DynObject {
    /// Checks if the object is of a specific type.
    ///
    /// For details of this operation see [`downcast_ref`](#method.downcast_ref).
    pub fn is<T: 'static>(&self) -> bool {
        self.downcast::<T>().is_some()
    }

    impl_object_helpers!(pub &Self);
}

impl Hash for DynObject {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if let Some(iter) = self.try_iter_pairs() {
            for (key, value) in iter {
                key.hash(state);
                value.hash(state);
            }
        }
    }
}

impl fmt::Display for DynObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.render(f)
    }
}

impl Enumerator {
    /// Returns the length if the object has one.
    pub fn len(&self) -> Option<usize> {
        Some(match self {
            Enumerator::Empty => 0,
            Enumerator::Values(v) => v.len(),
            Enumerator::Str(v) => v.len(),
            Enumerator::Iter(i) => match i.size_hint() {
                (a, Some(b)) if a == b => a,
                _ => return None,
            },
            Enumerator::RevIter(i) => match i.size_hint() {
                (a, Some(b)) if a == b => a,
                _ => return None,
            },
            Enumerator::Seq(v) => *v,
            Enumerator::NonEnumerable => return None,
        })
    }

    /// Checks if the object is considered empty.
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }
}

impl<T: Into<Value> + Clone + Send + Sync + fmt::Debug> Object for Vec<T> {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(some!(key.as_usize())).cloned().map(|v| v.into())
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Seq(Vec::len(self))
    }
}

impl Object for ValueMap {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(key).cloned()
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        self.mapped_rev_enumerator(|this| Box::new(this.keys().cloned()))
    }
}

impl<K, V> Object for HashMap<K, V>
where
    K: Borrow<str>
        + AsRef<str>
        + PartialEq
        + Eq
        + Hash
        + Clone
        + Send
        + Sync
        + fmt::Debug
        + 'static,
    V: Into<Value> + Clone + Send + Sync + fmt::Debug + 'static,
{
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(some!(key.as_str())).cloned().map(|v| v.into())
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        self.mapped_enumerator(|this| {
            Box::new(this.keys().map(|k| intern(k.as_ref())).map(Value::from))
        })
    }
}
