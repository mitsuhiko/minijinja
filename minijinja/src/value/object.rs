use std::any::{Any, TypeId};
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use crate::error::{Error, ErrorKind};
use crate::value::{intern, Value};
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
    /// If not implemented behavior for an object is [`ObjectKind::Plain`]
    /// which just means that it's stringifyable and potentially can be
    /// called or has methods.
    ///
    /// For more information see [`ObjectKind`].
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Plain
    }

    /// Called when the engine tries to call a method on the object.
    ///
    /// It's the responsibility of the implementer to ensure that an
    /// error is generated if an invalid method is invoked.  If the method
    /// is not known an [`ErrorKind::UnknownMethod`] error must be returned.
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
    /// [`from_seq_object`](Value::from_seq_object) and [`from_struct_object`](Value::from_struct_object).
    ///
    /// Because this method works also for objects that only implement [`StructObject`]
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
    /// It also works with [`SeqObject`] or [`StructObject`]:
    ///
    /// ```rust
    /// # use minijinja::value::{Value, SeqObject};
    ///
    /// struct Thing {
    ///     id: usize,
    /// }
    ///
    /// impl SeqObject for Thing {
    ///     fn get_item(&self, idx: usize) -> Option<Value> {
    ///         (idx < 3).then(|| Value::from(idx))
    ///     }
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
        } else if type_id == TypeId::of::<SimpleSeqObject<T>>() {
            // SAFETY: type type id check ensures this type cast is correct
            let wrapper = unsafe { &*(self as *const dyn Object as *const SimpleSeqObject<T>) };
            return Some(&wrapper.0);
        } else if type_id == TypeId::of::<SimpleStructObject<T>>() {
            // SAFETY: type type id check ensures this type cast is correct
            let wrapper = unsafe { &*(self as *const dyn Object as *const SimpleStructObject<T>) };
            return Some(&wrapper.0);
        }
        None
    }

    /// Checks if the object is of a specific type.
    ///
    /// For details of this operation see [`downcast_ref`](#method.downcast_ref).
    pub fn is<T: 'static>(&self) -> bool {
        let type_id = (*self).type_id();
        type_id == TypeId::of::<T>()
            || type_id == TypeId::of::<SimpleSeqObject<T>>()
            || type_id == TypeId::of::<SimpleStructObject<T>>()
    }
}

impl<T: Object> Object for Arc<T> {
    #[inline]
    fn kind(&self) -> ObjectKind<'_> {
        T::kind(self)
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

/// A kind defines the object's behavior.
///
/// When a dynamic [`Object`] is implemented, it can be of one of the kinds
/// here.  The default behavior will be a [`Plain`](Self::Plain) object which
/// doesn't do much other than that it can be printed.  For an object to turn
/// into a [struct](Self::Struct) or [sequence](Self::Seq) the necessary kind
/// has to be returned with a pointer to itself.
///
/// Today object's can have the behavior of structs and sequences but this
/// might expand in the future.  It does mean that not all types of values can
/// be represented by objects.
#[non_exhaustive]
pub enum ObjectKind<'a> {
    /// This object is a plain object.
    ///
    /// Such an object has no attributes but it might be callable and it
    /// can be stringified.  When serialized it's serialized in it's
    /// stringified form.
    Plain,

    /// This object is a sequence.
    ///
    /// Requires that the object implements [`SeqObject`].
    Seq(&'a dyn SeqObject),

    /// This object is a struct (map with string keys).
    ///
    /// Requires that the object implements [`StructObject`].
    Struct(&'a dyn StructObject),

    /// This object is an iterator that yields new values.
    ///
    /// Requires that the object implements [`IteratorObject`].  It's not
    /// recommended to implement this, instead one should directly pass
    /// iterators to [`Value::from_iterator`].
    Iterator(&'a dyn IteratorObject),
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
/// by using [`Value::from_struct_object`].  In that case only [`StructObject`]
/// needs to be implemented and the value will provide default implementations
/// for stringification and debug printing.
///
/// ```
/// use minijinja::value::{Value, StructObject};
///
/// struct Point(f32, f32, f32);
///
/// impl StructObject for Point {
///     fn get_field(&self, name: &str) -> Option<Value> {
///         match name {
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
/// let value = Value::from_struct_object(Point(1.0, 2.5, 3.0));
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
///     fn get_field(&self, name: &str) -> Option<Value> {
///         match name {
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
/// use minijinja::value::{Value, StructObject};
///
/// pub struct DynamicContext {
///     magic: i32,
/// }
///
/// impl StructObject for DynamicContext {
///     fn get_field(&self, field: &str) -> Option<Value> {
///         match field {
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
/// let ctx = Value::from_struct_object(DynamicContext { magic: 42 });
/// let rv = tmpl.render(ctx)?;
/// # Ok(()) }
/// ```
///
/// One thing of note here is that in the above example `env` would be re-created every
/// time the template needs it.  A better implementation would cache the value after it
/// was created first.
pub trait StructObject: Send + Sync {
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
    fn get_field(&self, name: &str) -> Option<Value>;

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
    fn fields(&self) -> Vec<Arc<str>> {
        self.static_fields()
            .into_iter()
            .flat_map(|fields| fields.iter().copied().map(intern))
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
}

impl<T: StructObject> StructObject for Arc<T> {
    #[inline]
    fn get_field(&self, name: &str) -> Option<Value> {
        T::get_field(self, name)
    }

    #[inline]
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        T::static_fields(self)
    }

    #[inline]
    fn fields(&self) -> Vec<Arc<str>> {
        T::fields(self)
    }

    #[inline]
    fn field_count(&self) -> usize {
        T::field_count(self)
    }
}

impl<'a, T: StructObject + ?Sized> StructObject for &'a T {
    #[inline]
    fn get_field(&self, name: &str) -> Option<Value> {
        T::get_field(self, name)
    }

    #[inline]
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        T::static_fields(self)
    }

    #[inline]
    fn fields(&self) -> Vec<Arc<str>> {
        T::fields(self)
    }

    #[inline]
    fn field_count(&self) -> usize {
        T::field_count(self)
    }
}

#[repr(transparent)]
pub struct SimpleSeqObject<T>(pub T);

impl<T: SeqObject + 'static> fmt::Display for SimpleSeqObject<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ok!(f.write_str("["));
        for (idx, val) in (&self.0 as &dyn SeqObject).iter().enumerate() {
            if idx > 0 {
                ok!(f.write_str(", "));
            }
            ok!(write!(f, "{val:?}"));
        }
        f.write_str("]")
    }
}

impl<T: SeqObject + 'static> fmt::Debug for SimpleSeqObject<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries((&self.0 as &dyn SeqObject).iter())
            .finish()
    }
}

impl<T: SeqObject + 'static> Object for SimpleSeqObject<T> {
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Seq(&self.0)
    }
}

#[repr(transparent)]
pub struct SimpleStructObject<T>(pub T);

impl<T: StructObject + 'static> fmt::Display for SimpleStructObject<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ok!(f.write_str("{"));
        for (idx, field) in self.0.fields().iter().enumerate() {
            if idx > 0 {
                ok!(f.write_str(", "));
            }
            let val = self.0.get_field(field).unwrap_or(Value::UNDEFINED);
            ok!(write!(f, "{field:?}: {val:?}"));
        }
        f.write_str("}")
    }
}

impl<T: StructObject + 'static> fmt::Debug for SimpleStructObject<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        for field in self.0.fields() {
            let value = self.0.get_field(&field).unwrap_or(Value::UNDEFINED);
            m.entry(&field, &value);
        }
        m.finish()
    }
}

impl<T: StructObject + 'static> Object for SimpleStructObject<T> {
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Struct(&self.0)
    }
}

/// Represents a dynamic iterable.
///
/// Iterators need to use interior mutability to function.
pub trait IteratorObject: Send + Sync {
    /// Produces the next value from the iterator.
    fn next_value(&self) -> Option<Value>;

    /// Returns the exact size of the iterator if known.
    ///
    /// An iterator must only return the length if it's known and correct.
    /// The default implementation returns `None`.  If the length is
    /// provided then `loop.revindex` and `loop.length` will return the
    /// correct information.
    fn iterator_len(&self) -> Option<usize> {
        None
    }
}

pub(crate) struct SimpleIteratorObject<I, T>(pub Mutex<I>)
where
    I: Iterator<Item = T> + Send + Sync + 'static,
    T: Into<Value> + 'static;

impl<I, T> fmt::Debug for SimpleIteratorObject<I, T>
where
    I: Iterator<Item = T> + Send + Sync + 'static,
    T: Into<Value> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Iterator").finish()
    }
}

impl<I, T> fmt::Display for SimpleIteratorObject<I, T>
where
    I: Iterator<Item = T> + Send + Sync + 'static,
    T: Into<Value> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<iterator>")
    }
}

impl<I, T> Object for SimpleIteratorObject<I, T>
where
    I: Iterator<Item = T> + Send + Sync + 'static,
    T: Into<Value> + 'static,
{
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Iterator(self)
    }
}

impl<I, T> IteratorObject for SimpleIteratorObject<I, T>
where
    I: Iterator<Item = T> + Send + Sync,
    T: Into<Value>,
{
    fn next_value(&self) -> Option<Value> {
        self.0.lock().unwrap().next().map(Into::into)
    }

    fn iterator_len(&self) -> Option<usize> {
        match self.0.lock().unwrap().size_hint() {
            (lower, Some(upper)) if lower == upper => Some(lower),
            _ => None,
        }
    }
}
