use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;
use std::collections::HashMap;

use crate::error::{Error, ErrorKind};
use crate::value::{intern, Value, ValueMap};
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
/// [`MapObject`].
pub trait Object: fmt::Display + fmt::Debug + Any + Sync + Send {
    /// Describes the kind of an object.
    ///
    /// If not implemented behavior for an object is [`ObjectKind::Plain`]
    /// which just means that it's stringifyable and potentially can be
    /// called or has methods.
    ///
    /// For more information see [`ObjectKind`].
    fn value(&self) -> Value {
        Value::from(())
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
            format!("object has no method named {name}"),
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

impl dyn Object {
    /// Returns some reference to the boxed object if it is of type `T`, or None if it isn’t.
    ///
    /// This is basically the "reverse" of [`from_object`](Value::from_object),
    /// [`from_seq_object`](Value::from_seq_object) and [`from_map_object`](Value::from_map_object).
    ///
    /// Because this method works also for objects that only implement [`MapObject`]
    /// and [`SeqObject`] these methods do not actually use trait bounds that are
    /// restricted to `Object`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use minijinja::value::{Value, Object};
    /// use std::fmt;
    ///
    /// #[derive(Debug)]
    /// struct Thing {
    ///     id: usize,
    /// }
    ///
    /// impl fmt::Display for Thing {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt::Debug::fmt(self, f)
    ///     }
    /// }
    ///
    /// impl Object for Thing {}
    ///
    /// let x_value = Value::from_object(Thing { id: 42 });
    /// let value_as_obj = x_value.as_object().unwrap();
    /// let thing = value_as_obj.downcast_ref::<Thing>().unwrap();
    /// assert_eq!(thing.id, 42);
    /// ```
    ///
    /// It also works with [`SeqObject`] or [`MapObject`]:
    ///
    /// ```rust
    /// # use minijinja::value::{Value, SeqObject};
    ///
    /// #[derive(Clone)]
    /// struct Thing {
    ///     id: usize,
    /// }
    ///
    /// impl SeqObject for Thing {
    ///     fn get_item(&self, idx: usize) -> Option<Value> {
    ///         (idx < 3).then(|| Value::from(idx))
    ///     }
    ///
    ///     fn item_count(&self) -> usize {
    ///         3
    ///     }
    /// }
    ///
    /// let x_value = Value::from_seq_object(Thing { id: 42 });
    /// let value_as_obj = x_value.as_object().unwrap();
    /// let thing = value_as_obj.downcast_ref::<Thing>().unwrap();
    /// assert_eq!(thing.id, 42);
    /// ```
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        let type_id = (*self).type_id();
        if type_id == TypeId::of::<T>() {
            // SAFETY: type type id check ensures this type cast is correct
            return Some(unsafe { &*(self as *const dyn Object as *const T) });
        }

        if type_id == TypeId::of::<Arc<dyn SeqObject>>() {
            let inner = unsafe {
                &*(self as *const dyn Object as *const Arc<dyn SeqObject>)
            };

            if SeqObject::type_id(&**inner) == TypeId::of::<T>() {
                // SAFETY: type type id check ensures this type cast is correct
                return Some(unsafe { &*(&**inner as *const dyn SeqObject as *const T) });
            }
        }

        if type_id == TypeId::of::<Arc<dyn MapObject>>() {
            let inner = unsafe {
                &*(self as *const dyn Object as *const Arc<dyn MapObject>)
            };

            if MapObject::type_id(&**inner) == TypeId::of::<T>() {
                // SAFETY: type type id check ensures this type cast is correct
                return Some(unsafe { &*(&**inner as *const dyn MapObject as *const T) });
            }
        }

        None
    }

    /// Checks if the object is of a specific type.
    ///
    /// For details of this operation see [`downcast_ref`](#method.downcast_ref).
    pub fn is<T: 'static>(&self) -> bool {
        let type_id = (*self).type_id();
        type_id == TypeId::of::<T>()
    }
}

impl<T: Object> Object for Arc<T> {
    #[inline]
    fn value(&self) -> Value {
        T::value(self)
    }

    #[inline]
    fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        T::call_method(self, state, name, args)
    }

    #[inline]
    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        T::call(self, state, args)
    }
}

/// Provides the behavior of an [`Object`] holding sequence of values.
///
/// An object holding a sequence of values (tuple, list etc.) can be
/// represented by this trait.
///
/// # Simplified Example
///
/// For sequences which do not need any special method behavior, the [`Value`]
/// type is capable of automatically constructing a wrapper [`Object`] by using
/// [`Value::from_seq_object`].  In that case only [`SeqObject`] needs to be
/// implemented and the value will provide default implementations for
/// stringification and debug printing.
///
/// ```
/// use minijinja::value::{Value, SeqObject};
///
/// #[derive(Clone)]
/// struct Point(f32, f32, f32);
///
/// impl SeqObject for Point {
///     fn get_item(&self, idx: usize) -> Option<Value> {
///         match idx {
///             0 => Some(Value::from(self.0)),
///             1 => Some(Value::from(self.1)),
///             2 => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn item_count(&self) -> usize {
///         3
///     }
/// }
///
/// let value = Value::from_seq_object(Point(1.0, 2.5, 3.0));
/// ```
///
/// # Full Example
///
/// This example shows how one can use [`SeqObject`] in conjunction
/// with a fully customized [`Object`].  Note that in this case not
/// only [`Object`] needs to be implemented, but also [`Debug`] and
/// [`Display`](std::fmt::Display) no longer come for free.
///
/// ```
/// use std::fmt;
/// use minijinja::value::{Value, Object, SeqObject};
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
///     fn value(&self) -> Value {
///         Value::from_seq_object(self.clone())
///     }
/// }
///
/// impl SeqObject for Point {
///     fn get_item(&self, idx: usize) -> Option<Value> {
///         match idx {
///             0 => Some(Value::from(self.0)),
///             1 => Some(Value::from(self.1)),
///             2 => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn item_count(&self) -> usize {
///         3
///     }
/// }
///
/// let value = Value::from_object(Point(1.0, 2.5, 3.0));
/// ```
pub trait SeqObject: Send + Sync {
    /// Looks up an item by index.
    ///
    /// Sequences should provide a value for all items in the range of `0..item_count`
    /// but the engine will assume that items within the range are `Undefined`
    /// if `None` is returned.
    fn get_item(&self, idx: usize) -> Option<Value>;

    /// Returns the number of items in the sequence.
    fn item_count(&self) -> usize;

    /// .
    #[doc(hidden)]
    fn type_id(&self) -> TypeId where Self: Any {
        <Self as Any>::type_id(self)
    }
}

impl dyn SeqObject + '_ {
    /// Convenient iterator over a [`SeqObject`].
    pub fn iter(&self) -> SeqObjectIter<'_> {
        SeqObjectIter {
            seq: self,
            range: 0..self.item_count(),
        }
    }
}

impl<'a> fmt::Debug for dyn SeqObject + 'a {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a> std::hash::Hash for dyn SeqObject + 'a {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.iter().for_each(|v| v.hash(state));
    }
}

impl<T: SeqObject> SeqObject for Arc<T> {
    #[inline]
    fn get_item(&self, idx: usize) -> Option<Value> {
        T::get_item(self, idx)
    }

    #[inline]
    fn item_count(&self) -> usize {
        T::item_count(self)
    }
}

impl<'a, T: SeqObject + ?Sized> SeqObject for &'a T {
    #[inline]
    fn get_item(&self, idx: usize) -> Option<Value> {
        T::get_item(self, idx)
    }

    #[inline]
    fn item_count(&self) -> usize {
        T::item_count(self)
    }
}

impl<T: Into<Value> + Send + Sync + Clone> SeqObject for [T] {
    #[inline(always)]
    fn get_item(&self, idx: usize) -> Option<Value> {
        self.get(idx).cloned().map(Into::into)
    }

    #[inline(always)]
    fn item_count(&self) -> usize {
        self.len()
    }
}

impl<T: Into<Value> + Send + Sync + Clone> SeqObject for Vec<T> {
    #[inline(always)]
    fn get_item(&self, idx: usize) -> Option<Value> {
        self.get(idx).cloned().map(Into::into)
    }

    #[inline(always)]
    fn item_count(&self) -> usize {
        self.len()
    }
}

/// Iterates over [`SeqObject`]
pub struct SeqObjectIter<'a> {
    seq: &'a dyn SeqObject,
    range: Range<usize>,
}

impl<'a> Iterator for SeqObjectIter<'a> {
    type Item = Value;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.range
            .next()
            .map(|idx| self.seq.get_item(idx).unwrap_or(Value::UNDEFINED))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl<'a> DoubleEndedIterator for SeqObjectIter<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.range
            .next_back()
            .map(|idx| self.seq.get_item(idx).unwrap_or(Value::UNDEFINED))
    }
}

impl<'a> ExactSizeIterator for SeqObjectIter<'a> {}

/// Provides the behavior of an [`Object`] holding a struct.
///
/// An basic object with the shape and behavior of a struct (that means a
/// map with string keys) can be represented by this trait.
///
/// # Simplified Example
///
/// For structs which do not need any special method behavior or methods, the
/// [`Value`] type is capable of automatically constructing a wrapper [`Object`]
/// by using [`Value::from_map_object`].  In that case only [`MapObject`]
/// needs to be implemented and the value will provide default implementations
/// for stringification and debug printing.
///
/// ```
/// use minijinja::value::{Value, MapObject};
///
/// #[derive(Clone)]
/// struct Point(f32, f32, f32);
///
/// impl MapObject for Point {
///     fn get_field(&self, key: &Value) -> Option<Value> {
///         match key.as_str()? {
///             "x" => Some(Value::from(self.0)),
///             "y" => Some(Value::from(self.1)),
///             "z" => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn static_fields(&self) -> Option<&'static [&'static str]> {
///         Some(&["x", "y", "z"][..])
///     }
/// }
///
/// let value = Value::from_map_object(Point(1.0, 2.5, 3.0));
/// ```
///
/// # Full Example
///
/// The following example shows how to implement a dynamic object which
/// represents a struct.  Note that in this case not only [`Object`] needs to be
/// implemented, but also [`Debug`] and [`Display`](std::fmt::Display) no longer
/// come for free.
///
/// ```
/// use std::fmt;
/// use minijinja::value::{Value, Object, MapObject};
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
///     fn value(&self) -> Value {
///         Value::from_map_object(self.clone())
///     }
/// }
///
/// impl MapObject for Point {
///     fn get_field(&self, key: &Value) -> Option<Value> {
///         match key.as_str()? {
///             "x" => Some(Value::from(self.0)),
///             "y" => Some(Value::from(self.1)),
///             "z" => Some(Value::from(self.2)),
///             _ => None,
///         }
///     }
///
///     fn static_fields(&self) -> Option<&'static [&'static str]> {
///         Some(&["x", "y", "z"][..])
///     }
/// }
///
/// let value = Value::from_object(Point(1.0, 2.5, 3.0));
/// ```
///
/// # Struct As context
///
/// Structs can also be used as template rendering context.  This has a lot of
/// benefits as it means that the serialization overhead can be largely to
/// completely avoided.  This means that even if templates take hundreds of
/// values, MiniJinja does not spend time eagerly converting them into values.
///
/// Here is a very basic example of how a template can be rendered with a dynamic
/// context.  Note that the implementation of [`fields`](Self::fields) is optional
/// for this to work.  It's in fact not used by the engine during rendering but
/// it is necessary for the [`debug()`](crate::functions::debug) function to be
/// able to show which values exist in the context.
///
/// ```
/// # fn main() -> Result<(), minijinja::Error> {
/// # use minijinja::Environment;
/// use minijinja::value::{Value, MapObject};
///
/// #[derive(Clone)]
/// pub struct DynamicContext {
///     magic: i32,
/// }
///
/// impl MapObject for DynamicContext {
///     fn get_field(&self, key: &Value) -> Option<Value> {
///         match key.as_str()? {
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
/// let ctx = Value::from_map_object(DynamicContext { magic: 42 });
/// let rv = tmpl.render(ctx)?;
/// # Ok(()) }
/// ```
///
/// One thing of note here is that in the above example `env` would be re-created every
/// time the template needs it.  A better implementation would cache the value after it
/// was created first.
pub trait MapObject: Send + Sync {
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
    fn get_field(&self, key: &Value) -> Option<Value>;

    /// If possible returns a static vector of field names.
    ///
    /// If fields cannot be statically determined, then this must return `None`
    /// and [`fields`](Self::fields) should be implemented instead.  If however
    /// this method is implemented, then [`fields`](Self::fields) should not be
    /// implemented as the default implementation dispatches to here, or it has
    /// to be implemented to match the output.
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        None
    }

    /// Returns a vector of field names.
    ///
    /// This should be implemented if [`static_fields`](Self::static_fields) cannot
    /// be implemented due to lifetime restrictions.  The default implementation
    /// converts the return value of [`static_fields`](Self::static_fields) into
    /// a compatible format automatically.
    fn fields(&self) -> Vec<Value> {
        self.static_fields()
            .into_iter()
            .flat_map(|fields| fields.iter().copied().map(intern))
            .map(Value::from)
            .collect()
    }

    /// Returns the number of fields.
    ///
    /// The default implementation uses [`fields`](Self::fields) and
    /// [`static_fields`](Self::static_fields) automatically.
    fn field_count(&self) -> usize {
        if let Some(fields) = self.static_fields() {
            fields.len()
        } else {
            self.fields().len()
        }
    }

    #[doc(hidden)]
    fn type_id(&self) -> TypeId where Self: Any {
        <Self as Any>::type_id(self)
    }
}

impl MapObject for ValueMap {
    #[inline]
    fn get_field(&self, key: &Value) -> Option<Value> {
        let v = self.get(key)?;
        Some(v.clone())
    }

    #[inline]
    fn fields(&self) -> Vec<Value> {
        self.keys()
            .cloned()
            .collect()
    }

    #[inline]
    fn field_count(&self) -> usize {
        self.len()
    }
}

impl fmt::Debug for dyn MapObject + '_ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<T: MapObject> MapObject for Arc<T> {
    #[inline]
    fn get_field(&self, key: &Value) -> Option<Value> {
        T::get_field(self, key)
    }

    #[inline]
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        T::static_fields(self)
    }

    #[inline]
    fn fields(&self) -> Vec<Value> {
        T::fields(self)
    }

    #[inline]
    fn field_count(&self) -> usize {
        T::field_count(self)
    }
}

impl<'a, T: MapObject + ?Sized> MapObject for &'a T {
    #[inline]
    fn get_field(&self, key: &Value) -> Option<Value> {
        T::get_field(self, key)
    }

    #[inline]
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        T::static_fields(self)
    }

    #[inline]
    fn fields(&self) -> Vec<Value> {
        T::fields(self)
    }

    #[inline]
    fn field_count(&self) -> usize {
        T::field_count(self)
    }
}

/// Iterates over [`MapObject`]
pub struct MapObjectIter<'a> {
    map: &'a dyn MapObject,
    keys: std::vec::IntoIter<Value>,
}

impl<'a> Iterator for MapObjectIter<'a> {
    type Item = (Value, Value);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let key = self.keys.next()?;
        let value = self.map.get_field(&key).unwrap_or(Value::UNDEFINED);
        Some((key, value))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}

impl<'a> DoubleEndedIterator for MapObjectIter<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        let key = self.keys.next_back()?;
        let value = self.map.get_field(&key).unwrap_or(Value::UNDEFINED);
        Some((key, value))
    }
}

impl dyn MapObject + '_ {
    /// Convenient iterator over a [`MapObject`].
    pub fn iter(&self) -> MapObjectIter<'_> {
        MapObjectIter {
            map: self,
            keys: self.fields().into_iter(),
        }
    }

    pub(crate) fn to_map(&self) -> ValueMap {
        self.iter().collect()
    }
}

impl<K, V> MapObject for HashMap<K, V>
    where K: Borrow<str> + AsRef<str> + PartialEq + Eq + std::hash::Hash + Clone + Send + Sync,
          V: Into<Value> + Clone + Send + Sync
{
    fn get_field<'a>(&'a self, key: &Value) -> Option<Value> {
        let key = key.as_str()?;
        self.get(key).map(|v| v.clone().into())
    }

    fn static_fields(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn fields(&self) -> Vec<Value> {
        self.keys()
            .map(|k| k.as_ref().into())
            .collect()
    }

    fn field_count(&self) -> usize {
        self.len()
    }
}

impl fmt::Display for dyn SeqObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

impl fmt::Display for dyn MapObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

impl Object for Arc<dyn SeqObject> {
    fn value(&self) -> Value {
        Value::from(self.clone())
    }
}

impl Object for Arc<dyn MapObject> {
    fn value(&self) -> Value {
        Value::from(self.clone())
    }
}
