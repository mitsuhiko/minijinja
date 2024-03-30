//! Provides a dynamic value type abstraction.
//!
//! This module gives access to a dynamically typed value which is used by
//! the template engine during execution.
//!
//! For the most part the existence of the value type can be ignored as
//! MiniJinja will perform the necessary conversions for you.  For instance
//! if you write a filter that converts a string you can directly declare the
//! filter to take a [`String`](std::string::String).  However for some more
//! advanced use cases it's useful to know that this type exists.
//!
//! # Basic Value Conversions
//!
//! Values are typically created via the [`From`] trait:
//!
//! ```
//! # use minijinja::value::Value;
//! let int_value = Value::from(42);
//! let none_value = Value::from(());
//! let true_value = Value::from(true);
//! ```
//!
//! Or via the [`FromIterator`] trait:
//!
//! ```
//! # use minijinja::value::Value;
//!
//! // collection into a sequence
//! let value: Value = (1..10).into_iter().collect();
//!
//! // collection into a map
//! let value: Value = [("key", "value")].into_iter().collect();
//! ```
//!
//! For certain types of iterators (`Send` + `Sync` + `'static`) it's also
//! possible to make the value lazily iterate over the value by using the
//! `Value::from_iterator` function instead:
//!
//! ```
//! // TODO: Did this create a use-once value?
//! // # use minijinja::value::Value;
//! // let value: Value = Value::from_iterator(1..10);
//! ```
//!
//! To to into the inverse directly the various [`TryFrom`](std::convert::TryFrom)
//! implementations can be used:
//!
//! ```
//! # use minijinja::value::Value;
//! use std::convert::TryFrom;
//! let v = u64::try_from(Value::from(42)).unwrap();
//! ```
//!
//! The special [`Undefined`](Value::UNDEFINED) value also exists but does not
//! have a rust equivalent.  It can be created via the [`UNDEFINED`](Value::UNDEFINED)
//! constant.
//!
//! # Serde Conversions
//!
//! MiniJinja will usually however create values via an indirection via [`serde`] when
//! a template is rendered or an expression is evaluated.  This can also be
//! triggered manually by using the [`Value::from_serializable`] method:
//!
//! ```
//! # use minijinja::value::Value;
//! let value = Value::from_serializable(&[1, 2, 3]);
//! ```
//!
//! The inverse of that operation is to pass a value directly as serializer to
//! a type that supports deserialization.  This requires the `deserialization`
//! feature.
//!
#![cfg_attr(
    feature = "deserialization",
    doc = r"
```
# use minijinja::value::Value;
use serde::Deserialize;
let value = Value::from(vec![1, 2, 3]);
let vec = Vec::<i32>::deserialize(value).unwrap();
```
"
)]
//!
//! # Value Function Arguments
//!
//! [Filters](crate::filters) and [tests](crate::tests) can take values as arguments
//! but optionally also rust types directly.  This conversion for function arguments
//! is performed by the [`FunctionArgs`] and related traits ([`ArgType`], [`FunctionResult`]).
//!
//! # Memory Management
//!
//! Values are immutable objects which are internally reference counted which
//! means they can be copied relatively cheaply.  Special care must be taken
//! so that cycles are not created to avoid causing memory leaks.
//!
//! # HTML Escaping
//!
//! MiniJinja inherits the general desire to be clever about escaping.  For this
//! purpose a value will (when auto escaping is enabled) always be escaped.  To
//! prevent this behavior the [`safe`](crate::filters::safe) filter can be used
//! in the template.  Outside of templates the [`Value::from_safe_string`] method
//! can be used to achieve the same result.
//!
//! # Dynamic Objects
//!
//! Values can also hold "dynamic" objects.  These are objects which implement the
//! [`Object`] trait and optionally [`SeqObject`] or [`MapObject`]  These can
//! be used to implement dynamic functionality such as stateful values and more.
//! Dynamic objects are internally also used to implement the special `loop`
//! variable or macros.
//!
//! To create a dynamic `Value` object, use [`Value::from_object`],
//! [`Value::from_object`], [`Value::from_object`] or the `From<Arc<T:
//! Object>>` implementations for `Value`:
//!
//! ```rust
//! # use std::sync::Arc;
//! # use minijinja::value::{Value, Object, DynObject};
//! #[derive(Debug)]
//! struct Foo;
//!
//! impl Object for Foo {
//!     /* implementation */
//! }
//!
//! let value = Value::from_object(Foo);
//! let value = Value::from_dyn_object(Arc::new(Foo));
//! ```

// this module is based on the content module in insta which in turn is based
// on the content module in serde::private::ser.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use serde::ser::{Serialize, Serializer};

use crate::error::{Error, ErrorKind, Result};
use crate::functions;
use crate::utils::OnDrop;
use crate::value::ops::as_f64;
use crate::value::serialize::transform;
use crate::vm::State;

pub use crate::value::argtypes::{from_args, ArgType, FunctionArgs, FunctionResult, Kwargs, Rest};
pub use crate::value::object::{
    DynObject, Enumeration, EnumerationIter, Object, ObjectExt, ObjectRepr,
};

#[macro_use]
mod type_erase;
mod argtypes;
#[cfg(feature = "deserialization")]
mod deserialize;
mod keyref;
pub(crate) mod merge_object;
pub(crate) mod namespace_object;
mod object;
pub(crate) mod ops;
mod serialize;

#[cfg(feature = "deserialization")]
pub use self::deserialize::ViaDeserialize;
use self::object::ObjectKeyValueIter;

// We use in-band signalling to roundtrip some internal values.  This is
// not ideal but unfortunately there is no better system in serde today.
const VALUE_HANDLE_MARKER: &str = "\x01__minijinja_ValueHandle";

#[cfg(feature = "preserve_order")]
pub(crate) type ValueMap = indexmap::IndexMap<Value, Value>;

#[cfg(not(feature = "preserve_order"))]
pub(crate) type ValueMap = std::collections::BTreeMap<Value, Value>;

#[inline(always)]
pub(crate) fn value_map_with_capacity(capacity: usize) -> ValueMap {
    #[cfg(not(feature = "preserve_order"))]
    {
        let _ = capacity;
        ValueMap::new()
    }
    #[cfg(feature = "preserve_order")]
    {
        ValueMap::with_capacity(crate::utils::untrusted_size_hint(capacity))
    }
}

thread_local! {
    static INTERNAL_SERIALIZATION: Cell<bool> = const { Cell::new(false) };

    // This should be an AtomicU64 but sadly 32bit targets do not necessarily have
    // AtomicU64 available.
    static LAST_VALUE_HANDLE: Cell<u32> = const { Cell::new(0) };
    static VALUE_HANDLES: RefCell<BTreeMap<u32, Value>> = RefCell::new(BTreeMap::new());
}

/// Function that returns true when serialization for [`Value`] is taking place.
///
/// MiniJinja internally creates [`Value`] objects from all values passed to the
/// engine.  It does this by going through the regular serde serialization trait.
/// In some cases users might want to customize the serialization specifically for
/// MiniJinja because they want to tune the object for the template engine
/// independently of what is normally serialized to disk.
///
/// This function returns `true` when MiniJinja is serializing to [`Value`] and
/// `false` otherwise.  You can call this within your own [`Serialize`]
/// implementation to change the output format.
///
/// This is particularly useful as serialization for MiniJinja does not need to
/// support deserialization.  So it becomes possible to completely change what
/// gets sent there, even at the cost of serializing something that cannot be
/// deserialized.
pub fn serializing_for_value() -> bool {
    INTERNAL_SERIALIZATION.with(|flag| flag.get())
}

/// Enables value optimizations.
///
/// If `key_interning` is enabled, this turns on that feature, otherwise
/// it becomes a noop.
#[inline(always)]
pub(crate) fn value_optimization() -> impl Drop {
    #[cfg(feature = "key_interning")]
    {
        crate::value::keyref::key_interning::use_string_cache()
    }
    #[cfg(not(feature = "key_interning"))]
    {
        OnDrop::new(|| {})
    }
}

fn mark_internal_serialization() -> impl Drop {
    let old = INTERNAL_SERIALIZATION.with(|flag| {
        let old = flag.get();
        flag.set(true);
        old
    });
    OnDrop::new(move || {
        if !old {
            INTERNAL_SERIALIZATION.with(|flag| flag.set(false));
        }
    })
}

/// Describes the kind of value.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[non_exhaustive]
pub enum ValueKind {
    /// The value is undefined
    Undefined,
    /// The value is the none singleton ([`()`])
    None,
    /// The value is a [`bool`]
    Bool,
    /// The value is a number of a supported type.
    Number,
    /// The value is a string.
    String,
    /// The value is a byte array.
    Bytes,
    /// The value is an array of other values.
    Seq,
    /// The value is a key/value mapping.
    Map,
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            ValueKind::Undefined => "undefined",
            ValueKind::None => "none",
            ValueKind::Bool => "bool",
            ValueKind::Number => "number",
            ValueKind::String => "string",
            ValueKind::Bytes => "bytes",
            ValueKind::Seq => "sequence",
            ValueKind::Map => "map",
        })
    }
}

/// Type type of string
#[derive(Copy, Clone, Debug)]
pub(crate) enum StringType {
    Normal,
    Safe,
}

/// Wraps an internal copyable value but marks it as packed.
///
/// This is used for `i128`/`u128` in the value repr to avoid
/// the excessive 16 byte alignment.
#[derive(Copy)]
#[repr(packed)]
pub(crate) struct Packed<T: Copy>(pub T);

impl<T: Copy> Clone for Packed<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Clone)]
pub(crate) enum ValueRepr {
    Undefined,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    None,
    Invalid(Arc<str>),
    U128(Packed<u128>),
    I128(Packed<i128>),
    // FIXME: Make Cow<'static, str>?
    String(Arc<str>, StringType),
    Bytes(Arc<Vec<u8>>),
    Object(DynObject),
}

impl fmt::Debug for ValueRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueRepr::Undefined => f.write_str("undefined"),
            ValueRepr::Bool(val) => fmt::Debug::fmt(val, f),
            ValueRepr::U64(val) => fmt::Debug::fmt(val, f),
            ValueRepr::I64(val) => fmt::Debug::fmt(val, f),
            ValueRepr::F64(val) => fmt::Debug::fmt(val, f),
            ValueRepr::None => f.write_str("none"),
            ValueRepr::Invalid(ref val) => write!(f, "<invalid value: {}>", val),
            ValueRepr::U128(val) => fmt::Debug::fmt(&{ val.0 }, f),
            ValueRepr::I128(val) => fmt::Debug::fmt(&{ val.0 }, f),
            ValueRepr::String(val, _) => fmt::Debug::fmt(val, f),
            ValueRepr::Bytes(val) => fmt::Debug::fmt(val, f),
            ValueRepr::Object(val) => fmt::Debug::fmt(val, f),
        }
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.0 {
            ValueRepr::None | ValueRepr::Undefined => 0u8.hash(state),
            ValueRepr::String(ref s, _) => s.hash(state),
            ValueRepr::Bool(b) => b.hash(state),
            ValueRepr::Invalid(s) => s.hash(state),
            ValueRepr::Bytes(b) => b.hash(state),
            ValueRepr::Object(d) => d.hash(state),
            ValueRepr::U64(_)
            | ValueRepr::I64(_)
            | ValueRepr::F64(_)
            | ValueRepr::U128(_)
            | ValueRepr::I128(_) => {
                if let Ok(val) = i64::try_from(self.clone()) {
                    val.hash(state)
                } else {
                    as_f64(self).map(|x| x.to_bits()).hash(state)
                }
            }
        }
    }
}

/// Represents a dynamically typed value in the template engine.
#[derive(Clone)]
pub struct Value(pub(crate) ValueRepr);

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (ValueRepr::None, ValueRepr::None) => true,
            (ValueRepr::Undefined, ValueRepr::Undefined) => true,
            (ValueRepr::String(ref a, _), ValueRepr::String(ref b, _)) => a == b,
            (ValueRepr::Bytes(a), ValueRepr::Bytes(b)) => a == b,
            _ => match ops::coerce(self, other) {
                Some(ops::CoerceResult::F64(a, b)) => a == b,
                Some(ops::CoerceResult::I128(a, b)) => a == b,
                Some(ops::CoerceResult::Str(a, b)) => a == b,
                None => {
                    if let (Some(a), Some(b)) = (self.as_object(), other.as_object()) {
                        let (a_keys, b_keys) = (a.enumeration(), b.enumeration());
                        if a_keys.len() != b_keys.len() {
                            return false;
                        }

                        a_keys
                            .into_iter()
                            .all(|key| a.get_value(&key) == b.get_value(&key))
                    } else {
                        false
                    }
                }
            },
        }
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn f64_total_cmp(left: f64, right: f64) -> Ordering {
    // this is taken from f64::total_cmp on newer rust versions
    let mut left = left.to_bits() as i64;
    let mut right = right.to_bits() as i64;
    left ^= (((left >> 63) as u64) >> 1) as i64;
    right ^= (((right >> 63) as u64) >> 1) as i64;
    left.cmp(&right)
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        let value_ordering = match (&self.0, &other.0) {
            (ValueRepr::None, ValueRepr::None) => Ordering::Equal,
            (ValueRepr::Undefined, ValueRepr::Undefined) => Ordering::Equal,
            (ValueRepr::String(ref a, _), ValueRepr::String(ref b, _)) => a.cmp(b),
            (ValueRepr::Bytes(a), ValueRepr::Bytes(b)) => a.cmp(b),
            _ => match ops::coerce(self, other) {
                Some(ops::CoerceResult::F64(a, b)) => f64_total_cmp(a, b),
                Some(ops::CoerceResult::I128(a, b)) => a.cmp(&b),
                Some(ops::CoerceResult::Str(a, b)) => a.cmp(b),
                None => match (self.kind(), other.kind()) {
                    (ValueKind::Seq, ValueKind::Seq) => match (self.try_iter(), other.try_iter()) {
                        (Ok(a), Ok(b)) => a.cmp(b),
                        _ => self.len().cmp(&other.len()),
                    },
                    (ValueKind::Map, ValueKind::Map) => match (self.as_object(), other.as_object())
                    {
                        (Some(a), Some(b)) => a.iter().cmp(b.iter()),
                        _ => self.len().cmp(&other.len()),
                    },
                    _ => Ordering::Equal,
                },
            },
        };
        value_ordering.then((self.kind() as usize).cmp(&(other.kind() as usize)))
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            ValueRepr::Undefined => Ok(()),
            ValueRepr::Bool(val) => val.fmt(f),
            ValueRepr::U64(val) => val.fmt(f),
            ValueRepr::I64(val) => val.fmt(f),
            ValueRepr::F64(val) => {
                if val.is_nan() {
                    f.write_str("NaN")
                } else if val.is_infinite() {
                    write!(f, "{}inf", if val.is_sign_negative() { "-" } else { "" })
                } else {
                    let mut num = val.to_string();
                    if !num.contains('.') {
                        num.push_str(".0");
                    }
                    write!(f, "{num}")
                }
            }
            ValueRepr::None => f.write_str("none"),
            ValueRepr::Invalid(ref val) => write!(f, "<invalid value: {}>", val),
            ValueRepr::I128(val) => write!(f, "{}", { val.0 }),
            ValueRepr::String(val, _) => write!(f, "{val}"),
            ValueRepr::Bytes(val) => write!(f, "{}", String::from_utf8_lossy(val)),
            ValueRepr::U128(val) => write!(f, "{}", { val.0 }),
            ValueRepr::Object(x) => write!(f, "{x}"),
        }
    }
}

impl Default for Value {
    fn default() -> Value {
        ValueRepr::Undefined.into()
    }
}

/// Intern a string.
///
/// When the `key_interning` feature is in used, then MiniJinja will attempt to
/// reuse strings in certain cases.  This function can be used to utilize the
/// same functionality.  There is no guarantee that a string will be interned
/// as there are heuristics involved for it.  Additionally the string interning
/// will only work during the template engine execution (eg: within filters etc.).
pub fn intern(s: &str) -> Arc<str> {
    #[cfg(feature = "key_interning")]
    {
        crate::value::keyref::key_interning::try_intern(s)
    }
    #[cfg(not(feature = "key_interning"))]
    {
        Arc::from(s.to_string())
    }
}

#[allow(clippy::len_without_is_empty)]
impl Value {
    /// The undefined value.
    ///
    /// This constant exists because the undefined type does not exist in Rust
    /// and this is the only way to construct it.
    pub const UNDEFINED: Value = Value(ValueRepr::Undefined);

    /// Creates a value from something that can be serialized.
    ///
    /// This is the method that MiniJinja will generally use whenever a serializable
    /// object is passed to one of the APIs that internally want to create a value.
    /// For instance this is what [`context!`](crate::context) and
    /// [`render`](crate::Template::render) will use.
    ///
    /// During serialization of the value, [`serializing_for_value`] will return
    /// `true` which makes it possible to customize serialization for MiniJinja.
    /// For more information see [`serializing_for_value`].
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let val = Value::from_serializable(&vec![1, 2, 3]);
    /// ```
    ///
    /// This method does not fail but it might return a value that is not valid.  Such
    /// values will when operated on fail in the template engine in most situations.
    /// This for instance can happen if the underlying implementation of [`Serialize`]
    /// fails.  There are also cases where invalid objects are silently hidden in the
    /// engine today.  This is for instance the case for when keys are used in hash maps
    /// that the engine cannot deal with.  Invalid values are considered an implementation
    /// detail.  There is currently no API to validate a value.
    ///
    /// If the `deserialization` feature is enabled then the inverse of this method
    /// is to use the [`Value`] type as serializer.  You can pass a value into the
    /// [`deserialize`](serde::Deserialize::deserialize) method of a type that supports
    /// serde deserialization.
    pub fn from_serializable<T: Serialize>(value: &T) -> Value {
        let _serialization_guard = mark_internal_serialization();
        let _optimization_guard = value_optimization();
        transform(value)
    }

    /// Creates a value from a safe string.
    ///
    /// A safe string is one that will bypass auto escaping.  For instance if you
    /// want to have the template engine render some HTML without the user having to
    /// supply the `|safe` filter, you can use a value of this type instead.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let val = Value::from_safe_string("<em>note</em>".into());
    /// ```
    pub fn from_safe_string(value: String) -> Value {
        ValueRepr::String(Arc::from(value), StringType::Safe).into()
    }

    /// Creates a value from a dynamic object.
    ///
    /// For more information see [`Object`].
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
    /// let val = Value::from_object(Thing { id: 42 });
    /// ```
    pub fn from_object<T: Object + Send + Sync + 'static>(value: T) -> Value {
        Value::from(ValueRepr::Object(DynObject::new(Arc::new(value))))
    }

    /// Like [`from_object`](Self::from_object) but for type erased dynamic objects.
    pub fn from_dyn_object<T: Into<DynObject>>(value: T) -> Value {
        Value::from(ValueRepr::Object(value.into()))
    }

    /// Creates a sequence that iterates over the given value.
    ///
    /// The function is invoked to create an iterator which is then turned into
    /// a sequence or iterator.
    pub fn from_object_iter<T, F>(object: T, maker: F) -> Value
    where
        T: Send + Sync + 'static,
        F: for<'a> Fn(&'a T) -> Box<dyn Iterator<Item = Value> + Send + Sync + 'a>
            + Send
            + Sync
            + 'static,
    {
        struct IterObject {
            iter: Box<dyn Iterator<Item = Value> + Send + Sync + 'static>,
            _object: DynObject,
        }

        impl Iterator for IterObject {
            type Item = Value;

            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next()
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
        }

        struct IterObjectMaker<T, F> {
            maker: F,
            object: T,
        }

        impl fmt::Debug for IterObject {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple("IterObject").finish()
            }
        }

        impl<T, F> fmt::Debug for IterObjectMaker<T, F> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("IterObjectMaker").finish()
            }
        }

        impl<T, F> Object for IterObjectMaker<T, F>
        where
            T: Send + Sync + 'static,
            F: for<'a> Fn(&'a T) -> Box<dyn Iterator<Item = Value> + Send + Sync + 'a>
                + Send
                + Sync
                + 'static,
        {
            fn repr(self: &Arc<Self>) -> ObjectRepr {
                ObjectRepr::Seq
            }

            // XXX: this seems very wrong.
            // see also https://github.com/mitsuhiko/minijinja/issues/453
            fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
                Some(key.clone())
            }

            fn enumeration(self: &Arc<Self>) -> Enumeration {
                let iter: Box<dyn Iterator<Item = Value> + Send + Sync + '_> =
                    (self.maker)(&self.object);
                let iter = unsafe { std::mem::transmute(iter) };
                let _object = DynObject::new(self.clone());
                Enumeration::Iterator(Box::new(IterObject { iter, _object }))
            }
        }

        Value::from_object(IterObjectMaker { maker, object })
    }

    /// Creates a callable value from a function.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let pow = Value::from_function(|a: u32| a * a);
    /// ```
    pub fn from_function<F, Rv, Args>(f: F) -> Value
    where
        // the crazy bounds here exist to enable borrowing in closures
        F: functions::Function<Rv, Args>
            + for<'a> functions::Function<Rv, <Args as FunctionArgs<'a>>::Output>,
        Rv: FunctionResult,
        Args: for<'a> FunctionArgs<'a>,
    {
        functions::BoxedFunction::new(f).to_value()
    }

    /// Returns the kind of the value.
    ///
    /// This can be used to determine what's in the value before trying to
    /// perform operations on it.
    pub fn kind(&self) -> ValueKind {
        match self.0 {
            ValueRepr::Undefined => ValueKind::Undefined,
            ValueRepr::Bool(_) => ValueKind::Bool,
            ValueRepr::U64(_) | ValueRepr::I64(_) | ValueRepr::F64(_) => ValueKind::Number,
            ValueRepr::None => ValueKind::None,
            ValueRepr::I128(_) => ValueKind::Number,
            ValueRepr::String(..) => ValueKind::String,
            ValueRepr::Bytes(_) => ValueKind::Bytes,
            ValueRepr::U128(_) => ValueKind::Number,
            // XXX: invalid values report themselves as maps which is a lie
            ValueRepr::Invalid(_) => ValueKind::Undefined,
            ValueRepr::Object(ref obj) => match obj.repr() {
                ObjectRepr::Map => ValueKind::Map,
                ObjectRepr::Seq => ValueKind::Seq,
            },
        }
    }

    /// Returns `true` if the value is a number.
    ///
    /// To convert a value into a primitive number, use [`TryFrom`] or [`TryInto`].
    pub fn is_number(&self) -> bool {
        matches!(
            self.0,
            ValueRepr::U64(_)
                | ValueRepr::I64(_)
                | ValueRepr::F64(_)
                | ValueRepr::I128(_)
                | ValueRepr::U128(_)
        )
    }

    /// Returns `true` if the map represents keyword arguments.
    pub fn is_kwargs(&self) -> bool {
        self.as_object().map_or(false, |o| o.as_kwargs().is_some())
    }

    /// Is this value true?
    pub fn is_true(&self) -> bool {
        match self.0 {
            ValueRepr::Bool(val) => val,
            ValueRepr::U64(x) => x != 0,
            ValueRepr::U128(x) => x.0 != 0,
            ValueRepr::I64(x) => x != 0,
            ValueRepr::I128(x) => x.0 != 0,
            ValueRepr::F64(x) => x != 0.0,
            ValueRepr::String(ref x, _) => !x.is_empty(),
            ValueRepr::Bytes(ref x) => !x.is_empty(),
            ValueRepr::None | ValueRepr::Undefined | ValueRepr::Invalid(_) => false,
            ValueRepr::Object(ref x) => !x.enumeration().is_empty(),
        }
    }

    /// Returns `true` if this value is safe.
    pub fn is_safe(&self) -> bool {
        matches!(&self.0, ValueRepr::String(_, StringType::Safe))
    }

    /// Returns `true` if this value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(&self.0, ValueRepr::Undefined)
    }

    /// Returns `true` if this value is none.
    pub fn is_none(&self) -> bool {
        matches!(&self.0, ValueRepr::None)
    }

    /// If the value is a string, return it.
    pub fn to_str(&self) -> Option<Arc<str>> {
        match &self.0 {
            ValueRepr::String(ref s, _) => Some(s.clone()),
            _ => None,
        }
    }

    /// If the value is a string, return it.
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            ValueRepr::String(ref s, _) => Some(s as &str),
            _ => None,
        }
    }

    /// If this is an i64 return it
    pub fn as_usize(&self) -> Option<usize> {
        usize::try_from(self.clone()).ok()
    }

    /// If this is an i64 return it
    pub fn as_i64(&self) -> Option<i64> {
        i64::try_from(self.clone()).ok()
    }

    /// Returns the bytes of this value if they exist.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0 {
            ValueRepr::String(ref s, _) => Some(s.as_bytes()),
            ValueRepr::Bytes(ref b) => Some(&b[..]),
            _ => None,
        }
    }

    /// If the value is an object, it's returned as [`Object`].
    pub fn as_object(&self) -> Option<DynObject> {
        match self.0 {
            ValueRepr::Object(ref dy) => Some(dy.clone()),
            _ => None,
        }
    }

    /// Returns the length of the contained value.
    ///
    /// Values without a length will return `None`.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let seq = Value::from(vec![1, 2, 3, 4]);
    /// assert_eq!(seq.len(), Some(4));
    /// ```
    pub fn len(&self) -> Option<usize> {
        match self.0 {
            ValueRepr::String(ref s, _) => Some(s.chars().count()),
            ValueRepr::Object(ref dy) => dy.enumeration().len(),
            _ => None,
        }
    }

    /// Looks up an attribute by attribute name.
    ///
    /// This this returns [`UNDEFINED`](Self::UNDEFINED) when an invalid key is
    /// resolved.  An error is returned if the value does not contain an object
    /// that has attributes.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// # fn test() -> Result<(), minijinja::Error> {
    /// let ctx = minijinja::context! {
    ///     foo => "Foo"
    /// };
    /// let value = ctx.get_attr("foo")?;
    /// assert_eq!(value.to_string(), "Foo");
    /// # Ok(()) }
    /// ```
    pub fn get_attr(&self, key: &str) -> Result<Value, Error> {
        let value = match self.0 {
            ValueRepr::Undefined => return Err(Error::from(ErrorKind::UndefinedError)),
            ValueRepr::Object(ref dy) => dy.get_value(&Value::from(key)),
            _ => None,
        };

        Ok(value.unwrap_or(Value::UNDEFINED))
    }

    /// Alternative lookup strategy without error handling exclusively for context
    /// resolution.
    ///
    /// The main difference is that the return value will be `None` if the value is
    /// unable to look up the key rather than returning `Undefined` and errors will
    /// also not be created.
    pub(crate) fn get_attr_fast(&self, key: &str) -> Option<Value> {
        match self.0 {
            ValueRepr::Object(ref dy) => dy.get_value(&Value::from(key)),
            _ => None,
        }
    }

    /// Looks up an index of the value.
    ///
    /// This is a shortcut for [`get_item`](Self::get_item).
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let seq = Value::from(vec![0u32, 1, 2]);
    /// let value = seq.get_item_by_index(1).unwrap();
    /// assert_eq!(value.try_into().ok(), Some(1));
    /// ```
    pub fn get_item_by_index(&self, idx: usize) -> Result<Value, Error> {
        self.get_item(&Value(ValueRepr::U64(idx as _)))
    }

    /// Looks up an item (or attribute) by key.
    ///
    /// This is similar to [`get_attr`](Self::get_attr) but instead of using
    /// a string key this can be any key.  For instance this can be used to
    /// index into sequences.  Like [`get_attr`](Self::get_attr) this returns
    /// [`UNDEFINED`](Self::UNDEFINED) when an invalid key is looked up.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let ctx = minijinja::context! {
    ///     foo => "Foo",
    /// };
    /// let value = ctx.get_item(&Value::from("foo")).unwrap();
    /// assert_eq!(value.to_string(), "Foo");
    /// ```
    pub fn get_item(&self, key: &Value) -> Result<Value, Error> {
        if let ValueRepr::Undefined = self.0 {
            Err(Error::from(ErrorKind::UndefinedError))
        } else {
            Ok(self.get_item_opt(key).unwrap_or(Value::UNDEFINED))
        }
    }

    /// Iterates over the value.
    ///
    /// Depending on the [`kind`](Self::kind) of the value the iterator
    /// has a different behavior.
    ///
    /// * [`ValueKind::Map`]: the iterator yields the keys of the map.
    /// * [`ValueKind::Seq`]: the iterator yields the items in the sequence.
    /// * [`ValueKind::None`] / [`ValueKind::Undefined`]: the iterator is empty.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// # fn test() -> Result<(), minijinja::Error> {
    /// let value = Value::from({
    ///     let mut m = std::collections::BTreeMap::new();
    ///     m.insert("foo", 42);
    ///     m.insert("bar", 23);
    ///     m
    /// });
    /// for key in value.try_iter()? {
    ///     let value = value.get_item(&key)?;
    ///     println!("{} = {}", key, value);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn try_iter(&self) -> Result<ValueIter<'_>, Error> {
        self.try_iter_owned().map(|inner| ValueIter {
            _marker: PhantomData,
            inner,
        })
    }

    /// Returns some reference to the boxed object if it is of type `T`, or None if it isn’t.
    ///
    /// This is basically the "reverse" of [`from_object`](Self::from_object),
    /// [`from_object`](Self::from_object) and
    /// [`from_object`](Self::from_object). It's also a shortcut for
    /// [`downcast_ref`](trait.Object.html#method.downcast_ref) on the return
    /// value of [`as_object`](Self::as_object).
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
    /// let thing = x_value.downcast_object_ref::<Thing>().unwrap();
    /// assert_eq!(thing.id, 42);
    /// ```
    pub fn downcast_object_ref<T: 'static>(&self) -> Option<&T> {
        match self.0 {
            ValueRepr::Object(ref o) => o.downcast_ref(),
            _ => None,
        }
    }

    pub fn downcast_object<T: 'static>(&self) -> Option<Arc<T>> {
        match self.0 {
            ValueRepr::Object(ref o) => o.downcast(),
            _ => None,
        }
    }

    pub(crate) fn get_item_opt(&self, key: &Value) -> Option<Value> {
        fn index(value: &Value, len: impl Fn() -> usize) -> Option<usize> {
            match value.as_i64().and_then(|v| isize::try_from(v).ok()) {
                Some(i) if i < 0 => len().checked_sub(i.unsigned_abs()),
                Some(i) => Some(i as usize),
                None => None,
            }
        }

        match self.0 {
            ValueRepr::Object(ref dy) => {
                let len = dy.enumeration().len();
                let idx = len.and_then(|n| index(key, || n));
                let value = idx.map(Value::from);
                dy.get_value(value.as_ref().unwrap_or(key))
            }
            ValueRepr::String(ref s, _) => {
                let idx = some!(index(key, || s.chars().count()));
                s.chars().nth(idx).map(Value::from)
            }
            _ => None,
        }
    }

    /// Calls the value directly.
    ///
    /// If the value holds a function or macro, this invokes it.  Note that in
    /// MiniJinja there is a separate namespace for methods on objects and callable
    /// items.  To call methods (which should be a rather rare occurrence) you
    /// have to use [`call_method`](Self::call_method).
    ///
    /// The `args` slice is for the arguments of the function call.  To pass
    /// keyword arguments use the [`Kwargs`] type.
    ///
    /// Usually the state is already available when it's useful to call this method,
    /// but when it's not available you can get a fresh template state straight
    /// from the [`Template`](crate::Template) via [`new_state`](crate::Template::new_state).
    ///
    /// ```
    /// # use minijinja::{Environment, value::{Value, Kwargs}};
    /// # let mut env = Environment::new();
    /// # env.add_template("foo", "").unwrap();
    /// # let tmpl = env.get_template("foo").unwrap();
    /// # let state = tmpl.new_state(); let state = &state;
    /// let func = Value::from_function(|v: i64, kwargs: Kwargs| {
    ///     v * kwargs.get::<i64>("mult").unwrap_or(1)
    /// });
    /// let rv = func.call(
    ///     state,
    ///     &[
    ///         Value::from(42),
    ///         Value::from(Kwargs::from_iter([("mult", Value::from(2))])),
    ///     ],
    /// ).unwrap();
    /// assert_eq!(rv, Value::from(84));
    /// ```
    ///
    /// With the [`args!`](crate::args) macro creating an argument slice is
    /// simplified:
    ///
    /// ```
    /// # use minijinja::{Environment, args, value::{Value, Kwargs}};
    /// # let mut env = Environment::new();
    /// # env.add_template("foo", "").unwrap();
    /// # let tmpl = env.get_template("foo").unwrap();
    /// # let state = tmpl.new_state(); let state = &state;
    /// let func = Value::from_function(|v: i64, kwargs: Kwargs| {
    ///     v * kwargs.get::<i64>("mult").unwrap_or(1)
    /// });
    /// let rv = func.call(state, args!(42, mult => 2)).unwrap();
    /// assert_eq!(rv, Value::from(84));
    /// ```
    pub fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        if let ValueRepr::Object(ref dy) = self.0 {
            dy.call(state, args)
        } else {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("value of type {} is not callable", self.kind()),
            ))
        }
    }

    /// Calls a method on the value.
    ///
    /// The name of the method is `name`, the arguments passed are in the `args`
    /// slice.
    pub fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        match self._call_method(state, name, args) {
            Ok(rv) => Ok(rv),
            Err(err) => {
                if err.kind() == ErrorKind::UnknownMethod {
                    if let Some(ref callback) = state.env().unknown_method_callback {
                        return callback(state, self, name, args);
                    }
                }
                Err(err)
            }
        }
    }

    fn _call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        if let Some(object) = self.as_object() {
            return object.call_method(state, name, args);
        }

        Err(Error::new(
            ErrorKind::UnknownMethod,
            format!("object has no method named {name}"),
        ))
    }

    /// Iterates over the value without holding a reference.
    pub(crate) fn try_iter_owned(&self) -> Result<OwnedValueIterator, Error> {
        let (iter_state, len) = match self.0 {
            ValueRepr::None | ValueRepr::Undefined => (ValueIteratorState::Empty, Some(0)),
            ValueRepr::String(ref s, _) => (
                ValueIteratorState::Chars(0, Arc::clone(s)),
                Some(s.chars().count()),
            ),
            ValueRepr::Object(ref obj) => {
                let len = obj.enumeration().len();
                (ValueIteratorState::Dyn(obj.repr(), obj.iter()), len)
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!("{} is not iterable", self.kind()),
                ))
            }
        };

        Ok(OwnedValueIterator { iter_state, len })
    }

    #[cfg(feature = "builtins")]
    pub(crate) fn get_path(&self, path: &str) -> Result<Value, Error> {
        let mut rv = self.clone();
        for part in path.split('.') {
            if let Ok(num) = part.parse::<usize>() {
                rv = ok!(rv.get_item_by_index(num));
            } else {
                rv = ok!(rv.get_attr(part));
            }
        }
        Ok(rv)
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // enable round tripping of values
        if serializing_for_value() {
            let handle = LAST_VALUE_HANDLE.with(|x| {
                // we are okay with overflowing the handle here because these values only
                // live for a very short period of time and it's not likely that you run out
                // of an entire u32 worth of handles in a single serialization operation.
                // This lets us stick the handle into a unit variant in the serde data model.
                let rv = x.get().wrapping_add(1);
                x.set(rv);
                rv
            });
            VALUE_HANDLES.with(|handles| handles.borrow_mut().insert(handle, self.clone()));
            return serializer.serialize_unit_variant(
                VALUE_HANDLE_MARKER,
                handle,
                VALUE_HANDLE_MARKER,
            );
        }

        match self.0 {
            ValueRepr::Bool(b) => serializer.serialize_bool(b),
            ValueRepr::U64(u) => serializer.serialize_u64(u),
            ValueRepr::I64(i) => serializer.serialize_i64(i),
            ValueRepr::F64(f) => serializer.serialize_f64(f),
            ValueRepr::None | ValueRepr::Undefined | ValueRepr::Invalid(_) => {
                serializer.serialize_unit()
            }
            ValueRepr::U128(u) => serializer.serialize_u128(u.0),
            ValueRepr::I128(i) => serializer.serialize_i128(i.0),
            ValueRepr::String(ref s, _) => serializer.serialize_str(s),
            ValueRepr::Bytes(ref b) => serializer.serialize_bytes(b),
            ValueRepr::Object(ref o) => match o.repr() {
                ObjectRepr::Seq => {
                    use serde::ser::SerializeSeq;
                    let enumeration = o.enumeration();
                    let mut seq = ok!(serializer.serialize_seq(enumeration.len()));
                    for item in o.values() {
                        ok!(seq.serialize_element(&item));
                    }

                    seq.end()
                }
                ObjectRepr::Map => {
                    use serde::ser::SerializeMap;
                    let mut map = ok!(serializer.serialize_map(None));
                    for (key, value) in o.iter() {
                        ok!(map.serialize_entry(&key, &value));
                    }

                    map.end()
                }
            },
        }
    }
}

/// Iterates over a value.
pub struct ValueIter<'a> {
    _marker: PhantomData<&'a Value>,
    inner: OwnedValueIterator,
}

impl<'a> Iterator for ValueIter<'a> {
    type Item = Value;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

pub(crate) struct OwnedValueIterator {
    iter_state: ValueIteratorState,
    len: Option<usize>,
}

impl Iterator for OwnedValueIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter_state.advance_state().map(|x| {
            if let Some(ref mut len) = self.len {
                *len -= 1;
            }
            x
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if let ValueIteratorState::Dyn(_, ref iter) = self.iter_state {
            return iter.size_hint();
        }

        (self.len.unwrap_or(0), self.len)
    }
}

impl fmt::Debug for OwnedValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueIterator").finish()
    }
}

enum ValueIteratorState {
    Empty,
    Chars(usize, Arc<str>),
    Dyn(ObjectRepr, ObjectKeyValueIter),
}

impl ValueIteratorState {
    fn advance_state(&mut self) -> Option<Value> {
        match self {
            ValueIteratorState::Empty => None,
            ValueIteratorState::Chars(offset, ref s) => {
                (s as &str)[*offset..].chars().next().map(|c| {
                    *offset += c.len_utf8();
                    Value::from(c)
                })
            }
            ValueIteratorState::Dyn(ObjectRepr::Map, iter) => iter.next().map(|kv| kv.0),
            ValueIteratorState::Dyn(ObjectRepr::Seq, iter) => iter.next().map(|kv| kv.1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use similar_asserts::assert_eq;

    #[test]
    fn test_dynamic_object_roundtrip() {
        use std::sync::atomic::{self, AtomicUsize};

        #[derive(Debug, Clone)]
        struct X(Arc<AtomicUsize>);

        impl fmt::Display for X {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.load(atomic::Ordering::Relaxed))
            }
        }

        impl Object for X {
            fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
                match key.as_str()? {
                    "value" => Some(Value::from(self.0.load(atomic::Ordering::Relaxed))),
                    _ => None,
                }
            }

            fn enumeration(self: &Arc<Self>) -> Enumeration {
                Enumeration::Static(&["value"])
            }

            fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, f)
            }
        }

        let x = Arc::new(X(Default::default()));
        let x_value = Value::from_dyn_object(x.clone());
        x.0.fetch_add(42, atomic::Ordering::Relaxed);
        let x_clone = Value::from_serializable(&x_value);
        x.0.fetch_add(23, atomic::Ordering::Relaxed);

        assert_eq!(x_value.to_string(), "65");
        assert_eq!(x_clone.to_string(), "65");
    }

    #[test]
    fn test_string_char() {
        let val = Value::from('a');
        assert_eq!(char::try_from(val).unwrap(), 'a');
        let val = Value::from("a");
        assert_eq!(char::try_from(val).unwrap(), 'a');
        let val = Value::from("wat");
        assert!(char::try_from(val).is_err());
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn test_sizes() {
        assert_eq!(std::mem::size_of::<Value>(), 24);
    }
}
