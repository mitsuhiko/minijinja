use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::ops::Range;
use std::sync::Arc;

use crate::error::{Error, ErrorKind, Result};
use crate::value::{intern, Value, ValueMap, ValueRepr};
use crate::vm::State;

/// A trait that represents a dynamic object.
pub trait Object: fmt::Debug {
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

    /// Returns the enumeration of the object.
    ///
    /// For more information see [`Enumeration`].  The default implementation
    /// returns the empty enumeration.
    fn enumeration(self: &Arc<Self>) -> Enumeration {
        Enumeration::Empty
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
    /// [`unknown_method_callback`](crate::Environment::set_unknonw_method_callback) of
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
    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct DbgRender<'a>(&'a Value);

        impl<'a> fmt::Debug for DbgRender<'a> {
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
                for key in self.enumeration() {
                    if let Some(value) = self.get_value(&key) {
                        dbg.entry(&DbgRender(&key), &DbgRender(&value));
                    }
                }

                dbg.finish()
            }
            ObjectRepr::Seq => {
                let mut dbg = f.debug_list();
                for key in self.enumeration() {
                    if let Some(value) = self.get_value(&key) {
                        dbg.entry(&DbgRender(&value));
                    }
                }

                dbg.finish()
            }
        }
    }
}

/// Provides utility methods for working with objects.
pub trait ObjectExt: Object + Send + Sync + 'static {
    /// Creates a new iterator enumeration that projects into the given object.
    fn mapped_enumeration<F>(self: &Arc<Self>, maker: F) -> Enumeration
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

        let iter: Box<dyn Iterator<Item = Value> + Send + Sync + '_> = maker(self);

        // SAFETY: this is safe because the `IterObject` will keep our object alive.
        let iter = unsafe { std::mem::transmute(iter) };
        let _object = self.clone();
        Enumeration::Iterator(Box::new(IterObject { iter, _object }))
    }
}

impl<T: Object + Send + Sync + 'static> ObjectExt for T {}

/// Utility type to enumerate an object.
///
/// The purpose of this type is to reveal the contents of an object.  Depending
/// on the shape of the object different values are appropriate.
#[non_exhaustive]
pub enum Enumeration {
    /// An empty object or sequences.
    Empty,
    /// A list of known values.
    ///
    /// If the object is a sequence these are the values, if the object is a
    /// map this are actually the keys.
    Values(Vec<Value>),
    /// A slice of static strings, usually to represent keys.
    Static(&'static [&'static str]),
    /// A dynamic iterator over some contents.
    Iterator(Box<dyn Iterator<Item = Value> + Send + Sync>),
    /// A dynamic iterator that also can be reversed.
    ReversibleIter(Box<dyn DoubleEndedIterator<Item = Value> + Send + Sync>),
    Range(Range<usize>),
}

/// Iterates over an enumeration.
pub struct EnumerationIter(EnumerationIterRepr);

enum EnumerationIterRepr {
    Values(std::vec::IntoIter<Value>),
    Static(std::slice::Iter<'static, &'static str>),
    Iterator(Box<dyn Iterator<Item = Value> + Send + Sync>),
    ReversibleIter(Box<dyn DoubleEndedIterator<Item = Value> + Send + Sync>),
    Range(Range<usize>),
    Empty,
}

/// Defines the natural representation of this object.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectRepr {
    /// serializes to {...} and over the enumeration, values
    Map,
    /// serializes to [...] over its values
    Seq,
}

type_erase! {
    pub trait Object: Send + Sync => DynObject(DynObjectVT) {
        fn repr(&self) -> ObjectRepr;

        fn get_value(&self, key: &Value) -> Option<Value>;

        fn enumeration(&self) -> Enumeration;

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

/// Iterates over [`Object`]
pub struct ObjectValueIter {
    enumeration: EnumerationIter,
    object: DynObject,
}

/// Iterates over [`Object`]
pub struct ObjectKeyValueIter {
    enumeration: EnumerationIter,
    object: DynObject,
}

impl DynObject {
    /// Checks if the object is of a specific type.
    ///
    /// For details of this operation see [`downcast_ref`](#method.downcast_ref).
    pub fn is<T: 'static>(&self) -> bool {
        self.downcast::<T>().is_some()
    }

    /// Iterator over the values of an object.
    // XXX: remove this
    // see https://github.com/mitsuhiko/minijinja/issues/456
    pub fn values(&self) -> ObjectValueIter {
        ObjectValueIter {
            enumeration: self.enumeration().into_iter(),
            object: self.clone(),
        }
    }

    /// Iterator over the keys, values of an object.
    // XXX: make this iterate over keys for maps and values for sequences and iterators.
    // see https://github.com/mitsuhiko/minijinja/issues/456
    pub fn iter(&self) -> ObjectKeyValueIter {
        ObjectKeyValueIter {
            enumeration: self.enumeration().into_iter(),
            object: self.clone(),
        }
    }
}

impl Hash for DynObject {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.iter().for_each(|(k, v)| {
            k.hash(state);
            v.hash(state);
        })
    }
}

impl fmt::Display for DynObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.render(f)
    }
}

impl Iterator for ObjectValueIter {
    type Item = Value;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.object.get_value(&self.enumeration.next()?)
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.enumeration.size_hint()
    }
}

impl DoubleEndedIterator for ObjectValueIter {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.object.get_value(&self.enumeration.next_back()?)
    }
}

impl Iterator for ObjectKeyValueIter {
    type Item = (Value, Value);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let key = self.enumeration.next()?;
        let value = self.object.get_value(&key)?;
        Some((key, value))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.enumeration.size_hint()
    }
}

impl DoubleEndedIterator for ObjectKeyValueIter {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        let key = self.enumeration.next_back()?;
        let value = self.object.get_value(&key)?;
        Some((key, value))
    }
}

impl Enumeration {
    /// Returns the length if the object has one.
    pub fn len(&self) -> Option<usize> {
        Some(match self {
            Enumeration::Values(v) => v.len(),
            Enumeration::Static(v) => v.len(),
            Enumeration::Iterator(i) => match i.size_hint() {
                (a, Some(b)) if a == b => a,
                _ => return None,
            },
            Enumeration::ReversibleIter(i) => match i.size_hint() {
                (a, Some(b)) if a == b => a,
                _ => return None,
            },
            Enumeration::Range(v) => v.len(),
            Enumeration::Empty => 0,
        })
    }

    /// Checks if the object is considered empty.
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }
}

impl IntoIterator for Enumeration {
    type Item = Value;

    type IntoIter = EnumerationIter;

    fn into_iter(self) -> Self::IntoIter {
        EnumerationIter(match self {
            Enumeration::Values(v) => EnumerationIterRepr::Values(v.into_iter()),
            Enumeration::Static(v) => EnumerationIterRepr::Static(v.iter()),
            Enumeration::Iterator(i) => EnumerationIterRepr::Iterator(i),
            Enumeration::ReversibleIter(i) => EnumerationIterRepr::ReversibleIter(i),
            Enumeration::Range(i) => EnumerationIterRepr::Range(i),
            Enumeration::Empty => EnumerationIterRepr::Empty,
        })
    }
}

impl Iterator for EnumerationIter {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            EnumerationIterRepr::Values(iter) => iter.next(),
            EnumerationIterRepr::Static(iter) => iter.next().copied().map(intern).map(Value::from),
            EnumerationIterRepr::Iterator(iter) => iter.next(),
            EnumerationIterRepr::ReversibleIter(iter) => iter.next(),
            EnumerationIterRepr::Range(iter) => iter.next().map(Value::from),
            EnumerationIterRepr::Empty => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            EnumerationIterRepr::Values(iter) => iter.size_hint(),
            EnumerationIterRepr::Static(iter) => iter.size_hint(),
            EnumerationIterRepr::Iterator(iter) => iter.size_hint(),
            EnumerationIterRepr::ReversibleIter(iter) => iter.size_hint(),
            EnumerationIterRepr::Range(iter) => iter.size_hint(),
            EnumerationIterRepr::Empty => (0, Some(0)),
        }
    }
}

// XXX: this trait implementation is not correct for iterators.
// Tracked in https://github.com/mitsuhiko/minijinja/issues/455
impl DoubleEndedIterator for EnumerationIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            EnumerationIterRepr::Values(iter) => iter.next_back(),
            EnumerationIterRepr::Static(iter) => {
                iter.next_back().copied().map(intern).map(Value::from)
            }
            EnumerationIterRepr::Iterator(iter) => iter.next(), // FIXME: ?
            EnumerationIterRepr::ReversibleIter(iter) => iter.next_back(),
            EnumerationIterRepr::Range(iter) => iter.next_back().map(Value::from),
            EnumerationIterRepr::Empty => None,
        }
    }
}

impl<T: Into<Value> + Clone + fmt::Debug> Object for Vec<T> {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(key.as_usize()?).cloned().map(|v| v.into())
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        Enumeration::Range(0..self.len())
    }
}

impl Object for ValueMap {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(key).cloned()
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        self.mapped_enumeration(|this| Box::new(this.keys().cloned()))
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
        self.get(key.as_str()?).cloned().map(|v| v.into())
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        self.mapped_enumeration(|this| {
            Box::new(this.keys().map(|k| intern(k.as_ref())).map(Value::from))
        })
    }
}
