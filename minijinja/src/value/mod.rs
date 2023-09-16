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
//! // collection into a sequence
//! let value: Value = (1..10).into_iter().collect();
//!
//! // collection into a map
//! let value: Value = [("key", "value")].into_iter().collect();
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
//! prupose a value will (when auto escaping is enabled) always be escaped.  To
//! prevent this behavior the [`safe`](crate::filters::safe) filter can be used
//! in the template.  Outside of templates the [`Value::from_safe_string`] method
//! can be used to achieve the same result.
//!
//! # Dynamic Objects
//!
//! Values can also hold "dynamic" objects.  These are objects which implement the
//! [`Object`] trait and optionally [`SeqObject`] or [`StructObject`]  These can
//! be used to implement dynamic functionality such as stateful values and more.
//! Dynamic objects are internally also used to implement the special `loop`
//! variable or macros.
//!
//! To create a dynamic `Value` object, use [`Value::from_object`],
//! [`Value::from_seq_object`], [`Value::from_struct_object`] or the `From<Arc<T:
//! Object>>` implementations for `Value`:
//!
//! ```rust
//! # use std::sync::Arc;
//! # use minijinja::value::{Value, Object};
//! #[derive(Debug)]
//! struct Foo;
//!
//! # impl std::fmt::Display for Foo {
//! #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }
//! # }
//! #
//! impl Object for Foo {
//!     /* implementation */
//! }
//!
//! let value = Value::from_object(Foo);
//! let value = Value::from(Arc::new(Foo));
//! let value = Value::from(Arc::new(Foo) as Arc<dyn Object>);
//! ```

// this module is based on the content module in insta which in turn is based
// on the content module in serde::private::ser.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use serde::ser::{Serialize, Serializer};

use crate::error::{Error, ErrorKind};
use crate::functions;
use crate::utils::OnDrop;
use crate::value::object::{SimpleSeqObject, SimpleStructObject};
use crate::value::ops::as_f64;
use crate::value::serialize::transform;
use crate::vm::State;

pub use crate::value::argtypes::{from_args, ArgType, FunctionArgs, FunctionResult, Kwargs, Rest};
pub use crate::value::object::{Object, ObjectKind, SeqObject, SeqObjectIter, StructObject};

mod argtypes;
#[cfg(feature = "deserialization")]
mod deserialize;
mod keyref;
pub(crate) mod merge_object;
mod object;
pub(crate) mod ops;
mod serialize;

#[cfg(feature = "deserialization")]
pub use self::deserialize::ViaDeserialize;

pub(crate) use crate::value::keyref::KeyRef;

// We use in-band signalling to roundtrip some internal values.  This is
// not ideal but unfortunately there is no better system in serde today.
const VALUE_HANDLE_MARKER: &str = "\x01__minijinja_ValueHandle";

#[cfg(feature = "preserve_order")]
pub(crate) type ValueMap = indexmap::IndexMap<KeyRef<'static>, Value>;

#[cfg(not(feature = "preserve_order"))]
pub(crate) type ValueMap = std::collections::BTreeMap<KeyRef<'static>, Value>;

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
    static INTERNAL_SERIALIZATION: Cell<bool> = Cell::new(false);

    // This should be an AtomicU64 but sadly 32bit targets do not necessarily have
    // AtomicU64 available.
    static LAST_VALUE_HANDLE: Cell<u32> = Cell::new(0);
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

/// The type of map
#[derive(Copy, Clone, Debug)]
pub(crate) enum MapType {
    /// A regular map
    Normal,
    /// A map representing keyword arguments
    Kwargs,
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
    String(Arc<str>, StringType),
    Bytes(Arc<Vec<u8>>),
    Seq(Arc<Vec<Value>>),
    Map(Arc<ValueMap>, MapType),
    Dynamic(Arc<dyn Object>),
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
            ValueRepr::Seq(val) => fmt::Debug::fmt(val, f),
            ValueRepr::Map(val, _) => fmt::Debug::fmt(val, f),
            ValueRepr::Dynamic(val) => fmt::Debug::fmt(val, f),
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
            ValueRepr::Seq(b) => b.hash(state),
            ValueRepr::Map(m, _) => m.iter().for_each(|(k, v)| {
                k.hash(state);
                v.hash(state);
            }),
            ValueRepr::Dynamic(d) => match d.kind() {
                ObjectKind::Plain => 0u8.hash(state),
                ObjectKind::Seq(s) => s.iter().for_each(|x| x.hash(state)),
                ObjectKind::Struct(s) => {
                    if let Some(fields) = s.static_fields() {
                        fields.iter().for_each(|k| {
                            k.hash(state);
                            s.get_field(k).hash(state);
                        });
                    } else {
                        s.fields().iter().for_each(|k| {
                            k.hash(state);
                            s.get_field(k).hash(state);
                        });
                    }
                }
            },
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
                    if let (Some(a), Some(b)) = (self.as_seq(), other.as_seq()) {
                        a.iter().eq(b.iter())
                    } else if self.kind() == ValueKind::Map && other.kind() == ValueKind::Map {
                        if self.len() != other.len() {
                            return false;
                        }
                        if let Ok(mut iter) = self.try_iter() {
                            iter.all(|x| self.get_item_opt(&x) == other.get_item_opt(&x))
                        } else {
                            false
                        }
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
                None => {
                    if let (Some(a), Some(b)) = (self.as_seq(), other.as_seq()) {
                        a.iter().cmp(b.iter())
                    } else if self.kind() == ValueKind::Map && other.kind() == ValueKind::Map {
                        if let (Ok(a), Ok(b)) = (self.try_iter(), other.try_iter()) {
                            a.map(|k| (k.clone(), self.get_item_opt(&k)))
                                .cmp(b.map(|k| (k.clone(), other.get_item_opt(&k))))
                        } else {
                            Ordering::Equal
                        }
                    } else {
                        Ordering::Equal
                    }
                }
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
            ValueRepr::Seq(values) => {
                ok!(f.write_str("["));
                for (idx, val) in values.iter().enumerate() {
                    if idx > 0 {
                        ok!(f.write_str(", "));
                    }
                    ok!(write!(f, "{val:?}"));
                }
                f.write_str("]")
            }
            ValueRepr::Map(m, _) => {
                ok!(f.write_str("{"));
                for (idx, (key, val)) in m.iter().enumerate() {
                    if idx > 0 {
                        ok!(f.write_str(", "));
                    }
                    ok!(write!(f, "{key:?}: {val:?}"));
                }
                f.write_str("}")
            }
            ValueRepr::U128(val) => write!(f, "{}", { val.0 }),
            ValueRepr::Dynamic(x) => write!(f, "{x}"),
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
    ///
    /// Objects are internally reference counted.  If you want to hold on to the
    /// `Arc` you can directly create the value from an arc'ed object:
    ///
    /// ```rust
    /// # use minijinja::value::{Value, Object};
    /// # #[derive(Debug)]
    /// # struct Thing { id: usize };
    /// # impl std::fmt::Display for Thing {
    /// #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    /// #         todo!();
    /// #     }
    /// # }
    /// # impl Object for Thing {}
    /// use std::sync::Arc;
    /// let val = Value::from(Arc::new(Thing { id: 42 }));
    /// ```
    pub fn from_object<T: Object>(value: T) -> Value {
        Value::from(Arc::new(value) as Arc<dyn Object>)
    }

    /// Creates a value from an owned [`SeqObject`].
    ///
    /// This is a simplified API for creating dynamic sequences
    /// without having to implement the entire [`Object`] protocol.
    pub fn from_seq_object<T: SeqObject + 'static>(value: T) -> Value {
        Value::from_object(SimpleSeqObject(value))
    }

    /// Creates a value from an owned [`StructObject`].
    ///
    /// This is a simplified API for creating dynamic structs
    /// without having to implement the entire [`Object`] protocol.
    pub fn from_struct_object<T: StructObject + 'static>(value: T) -> Value {
        Value::from_object(SimpleStructObject(value))
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
            ValueRepr::Seq(_) => ValueKind::Seq,
            ValueRepr::Map(..) => ValueKind::Map,
            // XXX: invalid values report themselves as maps which is a lie
            ValueRepr::Invalid(_) => ValueKind::Map,
            ValueRepr::Dynamic(ref dy) => match dy.kind() {
                // XXX: basic objects should probably not report as map
                ObjectKind::Plain => ValueKind::Map,
                ObjectKind::Seq(_) => ValueKind::Seq,
                ObjectKind::Struct(_) => ValueKind::Map,
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
        matches!(self.0, ValueRepr::Map(_, MapType::Kwargs))
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
            ValueRepr::Seq(ref x) => !x.is_empty(),
            ValueRepr::Map(ref x, _) => !x.is_empty(),
            ValueRepr::Dynamic(ref x) => match x.kind() {
                ObjectKind::Plain => true,
                ObjectKind::Seq(s) => s.item_count() != 0,
                ObjectKind::Struct(s) => s.field_count() != 0,
            },
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
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            ValueRepr::String(ref s, _) => Some(s as &str),
            _ => None,
        }
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
    pub fn as_object(&self) -> Option<&dyn Object> {
        match self.0 {
            ValueRepr::Dynamic(ref dy) => Some(&**dy as &dyn Object),
            _ => None,
        }
    }

    /// If the value is a sequence it's returned as [`SeqObject`].
    pub fn as_seq(&self) -> Option<&dyn SeqObject> {
        match self.0 {
            ValueRepr::Seq(ref v) => return Some(&**v as &dyn SeqObject),
            ValueRepr::Dynamic(ref dy) => {
                if let ObjectKind::Seq(seq) = dy.kind() {
                    return Some(seq);
                }
            }
            _ => {}
        }
        None
    }

    /// If the value is a struct, return it as [`StructObject`].
    pub fn as_struct(&self) -> Option<&dyn StructObject> {
        if let ValueRepr::Dynamic(ref dy) = self.0 {
            if let ObjectKind::Struct(s) = dy.kind() {
                return Some(s);
            }
        }
        None
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
            ValueRepr::Map(ref items, _) => Some(items.len()),
            ValueRepr::Seq(ref items) => Some(items.len()),
            ValueRepr::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Plain => None,
                ObjectKind::Seq(s) => Some(s.item_count()),
                ObjectKind::Struct(s) => Some(s.field_count()),
            },
            _ => None,
        }
    }

    /// Looks up an attribute by attribute name.
    ///
    /// This this returns [`UNDEFINED`](Self::UNDEFINED) when an invalid key is
    /// resolved.  An error is returned when if the value does not contain an object
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
        Ok(match self.0 {
            ValueRepr::Undefined => return Err(Error::from(ErrorKind::UndefinedError)),
            ValueRepr::Map(ref items, _) => items.get(&KeyRef::Str(key)).cloned(),
            ValueRepr::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Struct(s) => s.get_field(key),
                ObjectKind::Plain | ObjectKind::Seq(_) => None,
            },
            _ => None,
        }
        .unwrap_or(Value::UNDEFINED))
    }

    /// Alternative lookup strategy without error handling exclusively for context
    /// resolution.
    ///
    /// The main difference is that the return value will be `None` if the value is
    /// unable to look up the key rather than returning `Undefined` and errors will
    /// also not be created.
    pub(crate) fn get_attr_fast(&self, key: &str) -> Option<Value> {
        match self.0 {
            ValueRepr::Map(ref items, _) => items.get(&KeyRef::Str(key)).cloned(),
            ValueRepr::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Struct(s) => s.get_field(key),
                ObjectKind::Plain | ObjectKind::Seq(_) => None,
            },
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
    /// [`from_seq_object`](Self::from_seq_object) and [`from_struct_object`](Self::from_struct_object).
    /// It's also a shortcut for [`downcast_ref`](trait.Object.html#method.downcast_ref)
    /// on the return value of [`as_object`](Self::as_object).
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
    /// let thing = x_value.downcast_object_ref::<Thing>().unwrap();
    /// assert_eq!(thing.id, 42);
    /// ```
    pub fn downcast_object_ref<T: 'static>(&self) -> Option<&T> {
        self.as_object().and_then(|x| x.downcast_ref())
    }

    pub(crate) fn get_item_opt(&self, key: &Value) -> Option<Value> {
        let key = KeyRef::Value(key.clone());

        let seq = match self.0 {
            ValueRepr::Map(ref items, _) => return items.get(&key).cloned(),
            ValueRepr::Seq(ref items) => &**items as &dyn SeqObject,
            ValueRepr::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Plain => return None,
                ObjectKind::Seq(s) => s,
                ObjectKind::Struct(s) => {
                    return if let Some(key) = key.as_str() {
                        s.get_field(key)
                    } else {
                        None
                    };
                }
            },
            ValueRepr::String(ref s, _) => {
                if let Some(idx) = key.as_i64() {
                    let idx = some!(isize::try_from(idx).ok());
                    let idx = if idx < 0 {
                        some!(s.chars().count().checked_sub(-idx as usize))
                    } else {
                        idx as usize
                    };
                    return s.chars().nth(idx).map(Value::from);
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        if let Some(idx) = key.as_i64() {
            let idx = some!(isize::try_from(idx).ok());
            let idx = if idx < 0 {
                some!(seq.item_count().checked_sub(-idx as usize))
            } else {
                idx as usize
            };
            seq.get_item(idx)
        } else {
            None
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
    /// keyword arguments use the [`Kwargs`](crate::value::Kwargs) type.
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
        if let ValueRepr::Dynamic(ref dy) = self.0 {
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
        match self.0 {
            ValueRepr::Dynamic(ref dy) => return dy.call_method(state, name, args),
            ValueRepr::Map(ref map, _) => {
                if let Some(value) = map.get(&KeyRef::Str(name)) {
                    return value.call(state, args);
                }
            }
            _ => {}
        }
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("object has no method named {name}"),
        ))
    }

    /// Iterates over the value without holding a reference.
    pub(crate) fn try_iter_owned(&self) -> Result<OwnedValueIterator, Error> {
        let (iter_state, len) = match self.0 {
            ValueRepr::None | ValueRepr::Undefined => (ValueIteratorState::Empty, 0),
            ValueRepr::String(ref s, _) => (
                ValueIteratorState::Chars(0, Arc::clone(s)),
                s.chars().count(),
            ),
            ValueRepr::Seq(ref seq) => (ValueIteratorState::Seq(0, Arc::clone(seq)), seq.len()),
            #[cfg(feature = "preserve_order")]
            ValueRepr::Map(ref items, _) => {
                (ValueIteratorState::Map(0, Arc::clone(items)), items.len())
            }
            #[cfg(not(feature = "preserve_order"))]
            ValueRepr::Map(ref items, _) => (
                ValueIteratorState::Map(
                    items.iter().next().map(|x| x.0.clone()),
                    Arc::clone(items),
                ),
                items.len(),
            ),
            ValueRepr::Dynamic(ref obj) => {
                match obj.kind() {
                    ObjectKind::Plain => (ValueIteratorState::Empty, 0),
                    ObjectKind::Seq(s) => (
                        ValueIteratorState::DynSeq(0, Arc::clone(obj)),
                        s.item_count(),
                    ),
                    ObjectKind::Struct(s) => {
                        // the assumption is that structs don't have excessive field counts
                        // and that most iterations go over all fields, so creating a
                        // temporary vector here is acceptable.
                        if let Some(fields) = s.static_fields() {
                            (ValueIteratorState::StaticStr(0, fields), fields.len())
                        } else {
                            let attrs = s.fields();
                            let attr_count = attrs.len();
                            (ValueIteratorState::ArcStr(0, attrs), attr_count)
                        }
                    }
                }
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
            ValueRepr::Seq(ref elements) => elements.serialize(serializer),
            ValueRepr::Map(ref entries, _) => {
                use serde::ser::SerializeMap;
                let mut map = ok!(serializer.serialize_map(Some(entries.len())));
                for (ref k, ref v) in entries.iter() {
                    ok!(map.serialize_entry(k, v));
                }
                map.end()
            }
            ValueRepr::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Plain => serializer.serialize_str(&dy.to_string()),
                ObjectKind::Seq(s) => {
                    use serde::ser::SerializeSeq;
                    let mut seq = ok!(serializer.serialize_seq(Some(s.item_count())));
                    for item in s.iter() {
                        ok!(seq.serialize_element(&item));
                    }
                    seq.end()
                }
                ObjectKind::Struct(s) => {
                    use serde::ser::SerializeMap;
                    let mut map = ok!(serializer.serialize_map(None));
                    if let Some(fields) = s.static_fields() {
                        for k in fields {
                            let v = s.get_field(k).unwrap_or(Value::UNDEFINED);
                            ok!(map.serialize_entry(k, &v));
                        }
                    } else {
                        for k in s.fields() {
                            let v = s.get_field(&k).unwrap_or(Value::UNDEFINED);
                            ok!(map.serialize_entry(&*k as &str, &v));
                        }
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
}

pub(crate) struct OwnedValueIterator {
    iter_state: ValueIteratorState,
    len: usize,
}

impl Iterator for OwnedValueIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter_state.advance_state().map(|x| {
            self.len -= 1;
            x
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl ExactSizeIterator for OwnedValueIterator {}

impl fmt::Debug for OwnedValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueIterator").finish()
    }
}

enum ValueIteratorState {
    Empty,
    Chars(usize, Arc<str>),
    Seq(usize, Arc<Vec<Value>>),
    StaticStr(usize, &'static [&'static str]),
    ArcStr(usize, Vec<Arc<str>>),
    DynSeq(usize, Arc<dyn Object>),
    #[cfg(not(feature = "preserve_order"))]
    Map(Option<KeyRef<'static>>, Arc<ValueMap>),
    #[cfg(feature = "preserve_order")]
    Map(usize, Arc<ValueMap>),
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
            ValueIteratorState::Seq(idx, items) => items
                .get(*idx)
                .map(|x| {
                    *idx += 1;
                    x
                })
                .cloned(),
            ValueIteratorState::StaticStr(idx, items) => items.get(*idx).map(|x| {
                *idx += 1;
                Value::from(intern(x))
            }),
            ValueIteratorState::ArcStr(idx, items) => items.get(*idx).map(|x| {
                *idx += 1;
                Value::from(x.clone())
            }),
            ValueIteratorState::DynSeq(idx, obj) => {
                if let ObjectKind::Seq(seq) = obj.kind() {
                    seq.get_item(*idx).map(|x| {
                        *idx += 1;
                        x
                    })
                } else {
                    unreachable!()
                }
            }
            #[cfg(feature = "preserve_order")]
            ValueIteratorState::Map(idx, map) => map.get_index(*idx).map(|x| {
                *idx += 1;
                x.0.as_value()
            }),
            #[cfg(not(feature = "preserve_order"))]
            ValueIteratorState::Map(ptr, map) => {
                if let Some(current) = ptr.take() {
                    let next = map.range(&current..).nth(1).map(|x| x.0.clone());
                    let rv = current.as_value();
                    *ptr = next;
                    Some(rv)
                } else {
                    None
                }
            }
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

        #[derive(Debug)]
        struct X(AtomicUsize);

        impl fmt::Display for X {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.load(atomic::Ordering::Relaxed))
            }
        }

        impl Object for X {
            fn kind(&self) -> ObjectKind<'_> {
                ObjectKind::Struct(self)
            }
        }

        impl crate::value::object::StructObject for X {
            fn get_field(&self, name: &str) -> Option<Value> {
                match name {
                    "value" => Some(Value::from(self.0.load(atomic::Ordering::Relaxed))),
                    _ => None,
                }
            }

            fn static_fields(&self) -> Option<&'static [&'static str]> {
                Some(&["value"][..])
            }
        }

        let x = Arc::new(X(Default::default()));
        let x_value = Value::from(x.clone());
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
