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
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        match self.enumeration() {
            Enumeration::Values(_) | Enumeration::Empty | Enumeration::Static(_) => ObjectRepr::Map,
            Enumeration::Iterator(_) | Enumeration::ReversibleIter(_) | Enumeration::Range(_) => {
                ObjectRepr::Seq
            }
        }
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let _ = key;
        None
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        Enumeration::Empty
    }

    fn call(
        self: &Arc<Self>,
        state: &State<'_, '_>,
        method: Option<&str>,
        args: &[Value],
    ) -> Result<Value> {
        let (_, _, _) = (state, method, args);
        if let Some(method) = method {
            if let Some(value) = self.get_value(&Value::from(method)) {
                return value.call(state, args);
            }

            Err(Error::new(
                ErrorKind::UnknownMethod,
                "object has no such method",
            ))
        } else {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                "object is not callable",
            ))
        }
    }

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

pub trait ObjectExt: Object + Send + Sync + 'static {
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
        let iter = unsafe { std::mem::transmute(iter) };
        let _object = self.clone();
        Enumeration::Iterator(Box::new(IterObject { iter, _object }))
    }
}

impl<T: Object + Send + Sync + 'static> ObjectExt for T {}

#[non_exhaustive]
pub enum Enumeration {
    Values(Vec<Value>),
    Static(&'static [&'static str]),
    Iterator(Box<dyn Iterator<Item = Value> + Send + Sync>),
    ReversibleIter(Box<dyn DoubleEndedIterator<Item = Value> + Send + Sync>),
    Range(Range<usize>),
    Empty,
}

pub enum EnumerationIter {
    Values(std::vec::IntoIter<Value>),
    Static(std::slice::Iter<'static, &'static str>),
    Iterator(Box<dyn Iterator<Item = Value> + Send + Sync>),
    ReversibleIter(Box<dyn DoubleEndedIterator<Item = Value> + Send + Sync>),
    Range(Range<usize>),
    Empty,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectRepr {
    /// serializes to {...} and over the enumeration, values
    Map,
    /// serializes to [...] over its values
    Seq,
}

impl ObjectRepr {
    pub fn is_seq(&self) -> bool {
        matches!(self, ObjectRepr::Seq)
    }
}

type_erase! {
    pub trait Object: Send + Sync => DynObject(DynObjectVT) {
        fn repr(&self) -> ObjectRepr;

        fn get_value(&self, key: &Value) -> Option<Value>;

        fn enumeration(&self) -> Enumeration;

        fn call(
            &self,
            state: &State<'_, '_>,
            method: Option<&str>,
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
    pub fn values(&self) -> ObjectValueIter {
        ObjectValueIter {
            enumeration: self.enumeration().into_iter(),
            object: self.clone(),
        }
    }

    /// Iterator over the keys, values of an object.
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
}

impl IntoIterator for Enumeration {
    type Item = Value;

    type IntoIter = EnumerationIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Enumeration::Values(v) => EnumerationIter::Values(v.into_iter()),
            Enumeration::Static(v) => EnumerationIter::Static(v.iter()),
            Enumeration::Iterator(i) => EnumerationIter::Iterator(i),
            Enumeration::ReversibleIter(i) => EnumerationIter::ReversibleIter(i),
            Enumeration::Range(i) => EnumerationIter::Range(i),
            Enumeration::Empty => EnumerationIter::Empty,
        }
    }
}

impl Iterator for EnumerationIter {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EnumerationIter::Values(iter) => iter.next(),
            EnumerationIter::Static(iter) => iter.next().copied().map(intern).map(Value::from),
            EnumerationIter::Iterator(iter) => iter.next(),
            EnumerationIter::ReversibleIter(iter) => iter.next(),
            EnumerationIter::Range(iter) => iter.next().map(Value::from),
            EnumerationIter::Empty => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            EnumerationIter::Values(iter) => iter.size_hint(),
            EnumerationIter::Static(iter) => iter.size_hint(),
            EnumerationIter::Iterator(iter) => iter.size_hint(),
            EnumerationIter::ReversibleIter(iter) => iter.size_hint(),
            EnumerationIter::Range(iter) => iter.size_hint(),
            EnumerationIter::Empty => (0, Some(0)),
        }
    }
}

impl DoubleEndedIterator for EnumerationIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            EnumerationIter::Values(iter) => iter.next_back(),
            EnumerationIter::Static(iter) => iter.next_back().copied().map(intern).map(Value::from),
            EnumerationIter::Iterator(iter) => iter.next(), // FIXME: ?
            EnumerationIter::ReversibleIter(iter) => iter.next_back(),
            EnumerationIter::Range(iter) => iter.next_back().map(Value::from),
            EnumerationIter::Empty => None,
        }
    }
}

impl<T: Into<Value> + Clone + fmt::Debug> Object for Vec<T> {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(key.as_usize()?).cloned().map(|v| v.into())
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        Enumeration::Range(0..self.len())
    }
}

impl Object for ValueMap {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

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
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(key.as_str()?).cloned().map(|v| v.into())
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        self.mapped_enumeration(|this| {
            Box::new(this.keys().map(|k| intern(k.as_ref())).map(Value::from))
        })
    }
}
