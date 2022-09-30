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
//! # Converting Values
//!
//! Values are typically created via the [`From`] trait:
//!
//! ```
//! # use minijinja::value::Value;
//! let value = Value::from(42);
//! ```
//!
//! MiniJinja will however create values via an indirection via [`serde`] when
//! a template is rendered or an expression is evaluated.  This can also be
//! triggered manually by using the [`Value::from_serializable`] method:
//!
//! ```
//! # use minijinja::value::Value;
//! let value = Value::from_serializable(&[1, 2, 3]);
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
//! [`Object`] trait.  These can be used to implement dynamic functionality such as
//! stateful values and more.  Dynamic objects are internally also used to implement
//! the special `loop` variable or macros.

// this module is based on the content module in insta which in turn is based
// on the content module in serde::private::ser.

use std::any::TypeId;
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::atomic::{self, AtomicBool, AtomicUsize};
use std::sync::Arc;

use serde::ser::{Serialize, Serializer};

use crate::error::{Error, ErrorKind};
use crate::functions;
use crate::key::{Key, StaticKey};
use crate::utils::OnDrop;
use crate::value::serialize::ValueSerializer;
use crate::vm::State;

pub use crate::value::argtypes::{from_args, ArgType, FunctionArgs, FunctionResult, Rest};
pub use crate::value::object::Object;

mod argtypes;
#[cfg(feature = "deserialization")]
mod deserialize;
mod object;
pub(crate) mod ops;
mod serialize;

#[cfg(test)]
use similar_asserts::assert_eq;

// We use in-band signalling to roundtrip some internal values.  This is
// not ideal but unfortunately there is no better system in serde today.
const VALUE_HANDLE_MARKER: &str = "\x01__minijinja_ValueHandle";

#[cfg(feature = "preserve_order")]
pub(crate) type ValueMap = indexmap::IndexMap<StaticKey, Value>;

#[cfg(not(feature = "preserve_order"))]
pub(crate) type ValueMap = std::collections::BTreeMap<StaticKey, Value>;

thread_local! {
    static INTERNAL_SERIALIZATION: AtomicBool = AtomicBool::new(false);
    static LAST_VALUE_HANDLE: AtomicUsize = AtomicUsize::new(0);
    static VALUE_HANDLES: RefCell<BTreeMap<usize, Value>> = RefCell::new(BTreeMap::new());
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
    INTERNAL_SERIALIZATION.with(|flag| flag.load(atomic::Ordering::Relaxed))
}

/// Enables a temporary code section within which some value
/// optimizations are enabled.  Currently this is exclusively
/// used to automatically intern keys when the `key_interning`
/// feature is enabled.
#[inline(always)]
pub(crate) fn with_value_optimization<R, F: FnOnce() -> R>(f: F) -> R {
    #[cfg(not(feature = "key_interning"))]
    {
        f()
    }
    #[cfg(feature = "key_interning")]
    {
        crate::key::key_interning::with(f)
    }
}

/// Executes code within the context of internal serialization
/// which causes the value type to enable the internal round
/// tripping serialization.
fn with_internal_serialization<R, F: FnOnce() -> R>(f: F) -> R {
    INTERNAL_SERIALIZATION.with(|flag| {
        let old = flag.load(atomic::Ordering::Relaxed);
        flag.store(true, atomic::Ordering::Relaxed);
        let _on_drop = OnDrop::new(|| {
            flag.store(old, atomic::Ordering::Relaxed);
        });
        with_value_optimization(f)
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
    /// The value is a character.
    Char,
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
        let ty = match *self {
            ValueKind::Undefined => "undefined",
            ValueKind::None => "none",
            ValueKind::Bool => "bool",
            ValueKind::Number => "number",
            ValueKind::Char => "char",
            ValueKind::String => "string",
            ValueKind::Bytes => "bytes",
            ValueKind::Seq => "sequence",
            ValueKind::Map => "map",
        };
        write!(f, "{}", ty)
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

#[derive(Clone)]
pub(crate) enum ValueRepr {
    Undefined,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    Char(char),
    None,
    U128(Arc<u128>),
    I128(Arc<i128>),
    String(Arc<String>, StringType),
    Bytes(Arc<Vec<u8>>),
    Seq(Arc<Vec<Value>>),
    Map(Arc<ValueMap>, MapType),
    Dynamic(Arc<dyn Object>),
}

impl fmt::Debug for ValueRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueRepr::Undefined => write!(f, "Undefined"),
            ValueRepr::Bool(val) => fmt::Debug::fmt(val, f),
            ValueRepr::U64(val) => fmt::Debug::fmt(val, f),
            ValueRepr::I64(val) => fmt::Debug::fmt(val, f),
            ValueRepr::F64(val) => fmt::Debug::fmt(val, f),
            ValueRepr::Char(val) => fmt::Debug::fmt(val, f),
            ValueRepr::None => write!(f, "None"),
            ValueRepr::U128(val) => fmt::Debug::fmt(val, f),
            ValueRepr::I128(val) => fmt::Debug::fmt(val, f),
            ValueRepr::String(val, _) => fmt::Debug::fmt(val, f),
            ValueRepr::Bytes(val) => fmt::Debug::fmt(val, f),
            ValueRepr::Seq(val) => fmt::Debug::fmt(val, f),
            ValueRepr::Map(val, _) => fmt::Debug::fmt(val, f),
            ValueRepr::Dynamic(val) => fmt::Debug::fmt(val, f),
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
            (ValueRepr::String(a, _), ValueRepr::String(b, _)) => a == b,
            (ValueRepr::Bytes(a), ValueRepr::Bytes(b)) => a == b,
            _ => match ops::coerce(self, other) {
                Some(ops::CoerceResult::F64(a, b)) => a == b,
                Some(ops::CoerceResult::I128(a, b)) => a == b,
                Some(ops::CoerceResult::String(a, b)) => a == b,
                None => false,
            },
        }
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self.0, &other.0) {
            (ValueRepr::None, ValueRepr::None) => Some(Ordering::Equal),
            (ValueRepr::String(a, _), ValueRepr::String(b, _)) => a.partial_cmp(b),
            (ValueRepr::Bytes(a), ValueRepr::Bytes(b)) => a.partial_cmp(b),
            _ => match ops::coerce(self, other) {
                Some(ops::CoerceResult::F64(a, b)) => a.partial_cmp(&b),
                Some(ops::CoerceResult::I128(a, b)) => a.partial_cmp(&b),
                Some(ops::CoerceResult::String(a, b)) => a.partial_cmp(&b),
                None => None,
            },
        }
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
            ValueRepr::Bool(val) => write!(f, "{}", val),
            ValueRepr::U64(val) => write!(f, "{}", val),
            ValueRepr::I64(val) => write!(f, "{}", val),
            ValueRepr::F64(val) => {
                if val.is_nan() {
                    write!(f, "NaN")
                } else if val.is_infinite() {
                    write!(f, "{}inf", if val.is_sign_negative() { "-" } else { "" })
                } else {
                    let mut num = val.to_string();
                    if !num.contains('.') {
                        num.push_str(".0");
                    }
                    write!(f, "{}", num)
                }
            }
            ValueRepr::Char(val) => write!(f, "{}", val),
            ValueRepr::None => write!(f, "none"),
            ValueRepr::I128(val) => write!(f, "{}", val),
            ValueRepr::String(val, _) => write!(f, "{}", val),
            ValueRepr::Bytes(val) => write!(f, "{}", String::from_utf8_lossy(val)),
            ValueRepr::Seq(values) => {
                ok!(write!(f, "["));
                for (idx, val) in values.iter().enumerate() {
                    if idx > 0 {
                        ok!(write!(f, ", "));
                    }
                    ok!(write!(f, "{:?}", val));
                }
                write!(f, "]")
            }
            ValueRepr::Map(m, _) => {
                ok!(write!(f, "{{"));
                for (idx, (key, val)) in m.iter().enumerate() {
                    if idx > 0 {
                        ok!(write!(f, ", "));
                    }
                    ok!(write!(f, "{:?}: {:?}", key, val));
                }
                write!(f, "}}")
            }
            ValueRepr::U128(val) => write!(f, "{}", val),
            ValueRepr::Dynamic(x) => write!(f, "{}", x),
        }
    }
}

impl Default for Value {
    fn default() -> Value {
        ValueRepr::None.into()
    }
}

#[allow(clippy::len_without_is_empty)]
impl Value {
    /// The undefined value
    pub const UNDEFINED: Value = Value(ValueRepr::Undefined);

    /// Creates a value from something that can be serialized.
    ///
    /// During serialization of the value, [`serializing_for_value`] will return
    /// `true` which makes it possible to customize serialization for MiniJinja.
    /// For more information see [`serializing_for_value`].
    pub fn from_serializable<T: Serialize>(value: &T) -> Value {
        with_internal_serialization(|| Serialize::serialize(value, ValueSerializer).unwrap())
    }

    /// Creates a value from a safe string.
    pub fn from_safe_string(value: String) -> Value {
        ValueRepr::String(Arc::new(value), StringType::Safe).into()
    }

    /// Creates a value from a reference counted dynamic object.
    pub(crate) fn from_rc_object<T: Object + 'static>(value: Arc<T>) -> Value {
        ValueRepr::Dynamic(value as Arc<dyn Object>).into()
    }

    /// Creates a value from a dynamic object.
    pub fn from_object<T: Object + 'static>(value: T) -> Value {
        Value::from_rc_object(Arc::new(value))
    }

    /// Creates a callable value from a function.
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

    /// Returns some reference to the boxed object if it is of type `T`, or None if it isn’t.
    ///
    /// This is basically the "reverse" of [`from_object`](Self::from_object).
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
    pub fn downcast_object_ref<T: Object>(&self) -> Option<&T> {
        if let ValueRepr::Dynamic(ref obj) = self.0 {
            if (**obj).type_id() == TypeId::of::<T>() {
                unsafe {
                    let raw: *const (dyn Object) = Arc::as_ptr(obj);
                    return (raw as *const u8 as *const T).as_ref();
                }
            }
        }
        None
    }

    /// Returns the value kind.
    pub fn kind(&self) -> ValueKind {
        match self.0 {
            ValueRepr::Undefined => ValueKind::Undefined,
            ValueRepr::Bool(_) => ValueKind::Bool,
            ValueRepr::U64(_) | ValueRepr::I64(_) | ValueRepr::F64(_) => ValueKind::Number,
            ValueRepr::Char(_) => ValueKind::Char,
            ValueRepr::None => ValueKind::None,
            ValueRepr::I128(_) => ValueKind::Number,
            ValueRepr::String(..) => ValueKind::String,
            ValueRepr::Bytes(_) => ValueKind::Bytes,
            ValueRepr::U128(_) => ValueKind::Number,
            ValueRepr::Seq(_) => ValueKind::Seq,
            ValueRepr::Map(..) | ValueRepr::Dynamic(_) => ValueKind::Map,
        }
    }

    /// Returns `true` if the map represents keyword arguments.
    pub fn is_kwargs(&self) -> bool {
        matches!(self.0, ValueRepr::Map(_, MapType::Kwargs))
    }

    /// If the value is a string, return it.
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            ValueRepr::String(ref s, _) => Some(s.as_str()),
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

    /// Like `as_str` but always stringifies the value.
    #[allow(unused)]
    pub(crate) fn to_cowstr(&self) -> Cow<'_, str> {
        match &self.0 {
            ValueRepr::String(ref s, _) => Cow::Borrowed(s.as_str()),
            _ => Cow::Owned(self.to_string()),
        }
    }

    /// Is this value true?
    pub fn is_true(&self) -> bool {
        match self.0 {
            ValueRepr::Bool(val) => val,
            ValueRepr::U64(x) => x != 0,
            ValueRepr::U128(ref x) => **x != 0,
            ValueRepr::I64(x) => x != 0,
            ValueRepr::I128(ref x) => **x != 0,
            ValueRepr::F64(x) => x != 0.0,
            ValueRepr::Char(x) => x != '\x00',
            ValueRepr::String(ref x, _) => !x.is_empty(),
            ValueRepr::Bytes(ref x) => !x.is_empty(),
            ValueRepr::None | ValueRepr::Undefined => false,
            ValueRepr::Seq(ref x) => !x.is_empty(),
            ValueRepr::Map(ref x, _) => !x.is_empty(),
            ValueRepr::Dynamic(_) => true,
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

    /// Returns the length of the contained value.
    pub fn len(&self) -> Option<usize> {
        match self.0 {
            ValueRepr::String(ref s, _) => Some(s.chars().count()),
            ValueRepr::Map(ref items, _) => Some(items.len()),
            ValueRepr::Seq(ref items) => Some(items.len()),
            ValueRepr::Dynamic(ref dy) => Some(dy.attributes().len()),
            _ => None,
        }
    }

    /// Looks up an attribute by attribute name.
    ///
    /// This this returns [`UNDEFINED`](Self::UNDEFINED) when an invalid key is
    /// resolved.  An error is returned when if the value does not contain an object
    /// that has attributes.
    pub fn get_attr(&self, key: &str) -> Result<Value, Error> {
        let value = match self.0 {
            ValueRepr::Map(ref items, _) => {
                let lookup_key = Key::Str(key);
                items.get(&lookup_key).cloned()
            }
            ValueRepr::Dynamic(ref dy) => dy.get_attr(key),
            ValueRepr::Undefined => {
                return Err(Error::from(ErrorKind::UndefinedError));
            }
            _ => None,
        };
        Ok(value.unwrap_or(Value::UNDEFINED))
    }

    /// Looks up an index of the value.
    ///
    /// This is a shortcut for [`get_item`](Self::get_item).
    pub fn get_item_by_index(&self, idx: usize) -> Result<Value, Error> {
        self.get_item(&Value(ValueRepr::U64(idx as _)))
    }

    /// Looks up an item (or attribute) by key.
    ///
    /// This is similar to [`get_attr`](Self::get_attr) but instead of using
    /// a string key this can be any key.  For instance this can be used to
    /// index into sequences.  Like [`get_attr`](Self::get_attr) this returns
    /// [`UNDEFINED`](Self::UNDEFINED) when an invalid key is looked up.
    pub fn get_item(&self, key: &Value) -> Result<Value, Error> {
        if let ValueRepr::Undefined = self.0 {
            Err(Error::from(ErrorKind::UndefinedError))
        } else {
            Ok(self.get_item_opt(key).unwrap_or(Value::UNDEFINED))
        }
    }

    fn get_item_opt(&self, key: &Value) -> Option<Value> {
        let key = some!(Key::from_borrowed_value(key).ok());

        match self.0 {
            ValueRepr::Map(ref items, _) => return items.get(&key).cloned(),
            ValueRepr::Seq(ref items) => {
                if let Key::I64(idx) = key {
                    let idx = some!(isize::try_from(idx).ok());
                    let idx = if idx < 0 {
                        items.len() - (-idx as usize)
                    } else {
                        idx as usize
                    };
                    return items.get(idx).cloned();
                }
            }
            ValueRepr::Dynamic(ref dy) => match key {
                Key::String(ref key) => return dy.get_attr(key),
                Key::Str(key) => return dy.get_attr(key),
                _ => {}
            },
            _ => {}
        }
        None
    }

    /// If the value is a sequence it's returned as slice.
    pub fn as_slice(&self) -> Result<&[Value], Error> {
        match self.0 {
            ValueRepr::Undefined | ValueRepr::None => Ok(&[][..]),
            ValueRepr::Seq(ref v) => Ok(&v[..]),
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("value of type {} is not a sequence", self.kind()),
            )),
        }
    }

    /// Calls the value directly.
    pub(crate) fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
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
    pub(crate) fn call_method(
        &self,
        state: &State,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        match self.0 {
            ValueRepr::Dynamic(ref dy) => return dy.call_method(state, name, args),
            ValueRepr::Map(ref map, _) => {
                if let Some(value) = map.get(&Key::Str(name)) {
                    return value.call(state, args);
                }
            }
            _ => {}
        }
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("object has no method named {}", name),
        ))
    }

    pub(crate) fn try_into_key(self) -> Result<StaticKey, Error> {
        match self.0 {
            ValueRepr::Bool(val) => Ok(Key::Bool(val)),
            ValueRepr::U64(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::U128(ref v) => TryFrom::try_from(**v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::I64(v) => Ok(Key::I64(v)),
            ValueRepr::I128(ref v) => TryFrom::try_from(**v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::Char(c) => Ok(Key::Char(c)),
            ValueRepr::String(ref s, _) => Ok(Key::String(s.clone())),
            _ => Err(ErrorKind::NonKey.into()),
        }
    }

    pub(crate) fn iter_as_str_map(&self) -> impl Iterator<Item = (&str, Value)> {
        match self.0 {
            ValueRepr::Map(ref m, _) => Box::new(
                m.iter()
                    .filter_map(|(k, v)| k.as_str().map(move |k| (k, v.clone()))),
            ) as Box<dyn Iterator<Item = _>>,
            ValueRepr::Dynamic(ref obj) => Box::new(
                obj.attributes()
                    .iter()
                    .filter_map(move |attr| Some((*attr, some!(obj.get_attr(attr))))),
            ) as Box<dyn Iterator<Item = _>>,
            _ => Box::new(None.into_iter()) as Box<dyn Iterator<Item = _>>,
        }
    }

    /// Iterates over the value.
    pub(crate) fn try_iter(&self) -> Result<ValueIterator, Error> {
        let (iter_state, len) = match self.0 {
            ValueRepr::None | ValueRepr::Undefined => (ValueIteratorState::Empty, 0),
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
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "object is not iterable",
                ))
            }
        };
        Ok(ValueIterator { iter_state, len })
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // enable round tripping of values
        if serializing_for_value() {
            use serde::ser::SerializeStruct;
            let handle = LAST_VALUE_HANDLE.with(|x| x.fetch_add(1, atomic::Ordering::Relaxed));
            VALUE_HANDLES.with(|handles| handles.borrow_mut().insert(handle, self.clone()));
            let mut s = ok!(serializer.serialize_struct(VALUE_HANDLE_MARKER, 1));
            ok!(s.serialize_field("handle", &handle));
            return s.end();
        }

        match self.0 {
            ValueRepr::Bool(b) => serializer.serialize_bool(b),
            ValueRepr::U64(u) => serializer.serialize_u64(u),
            ValueRepr::I64(i) => serializer.serialize_i64(i),
            ValueRepr::F64(f) => serializer.serialize_f64(f),
            ValueRepr::Char(c) => serializer.serialize_char(c),
            ValueRepr::None => serializer.serialize_unit(),
            ValueRepr::Undefined => serializer.serialize_unit(),
            ValueRepr::U128(ref u) => serializer.serialize_u128(**u),
            ValueRepr::I128(ref i) => serializer.serialize_i128(**i),
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
            ValueRepr::Dynamic(ref n) => {
                use serde::ser::SerializeMap;
                let fields = n.attributes();
                let mut s = ok!(serializer.serialize_map(Some(fields.len())));
                for k in fields {
                    let v = n.get_attr(k).unwrap_or(Value::UNDEFINED);
                    ok!(s.serialize_entry(k, &v));
                }
                s.end()
            }
        }
    }
}

pub(crate) struct ValueIterator {
    iter_state: ValueIteratorState,
    len: usize,
}

impl Iterator for ValueIterator {
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

impl ExactSizeIterator for ValueIterator {}

impl fmt::Debug for ValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueIterator").finish()
    }
}

enum ValueIteratorState {
    Empty,
    Seq(usize, Arc<Vec<Value>>),
    #[cfg(not(feature = "preserve_order"))]
    Map(Option<StaticKey>, Arc<ValueMap>),
    #[cfg(feature = "preserve_order")]
    Map(usize, Arc<ValueMap>),
}

impl ValueIteratorState {
    fn advance_state(&mut self) -> Option<Value> {
        match self {
            ValueIteratorState::Empty => None,
            ValueIteratorState::Seq(idx, items) => items
                .get(*idx)
                .map(|x| {
                    *idx += 1;
                    x
                })
                .cloned(),
            #[cfg(feature = "preserve_order")]
            ValueIteratorState::Map(idx, map) => map.get_index(*idx).map(|x| {
                *idx += 1;
                Value::from(x.0.clone())
            }),
            #[cfg(not(feature = "preserve_order"))]
            ValueIteratorState::Map(ptr, map) => {
                if let Some(current) = ptr.take() {
                    let next = map.range(&current..).nth(1).map(|x| x.0.clone());
                    let rv = Value::from(current);
                    *ptr = next;
                    Some(rv)
                } else {
                    None
                }
            }
        }
    }
}

#[test]
fn test_dynamic_object_roundtrip() {
    #[derive(Debug)]
    struct X(AtomicUsize);

    impl fmt::Display for X {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0.load(atomic::Ordering::Relaxed))
        }
    }

    impl Object for X {
        fn get_attr(&self, name: &str) -> Option<Value> {
            match name {
                "value" => Some(Value::from(self.0.load(atomic::Ordering::Relaxed))),
                _ => None,
            }
        }

        fn attributes(&self) -> &'static [&'static str] {
            &["value"]
        }
    }

    let x = Arc::new(X(Default::default()));
    let x_value = Value::from_rc_object(x.clone());
    x.0.fetch_add(42, atomic::Ordering::Relaxed);
    let x_clone = Value::from_serializable(&x_value);
    x.0.fetch_add(23, atomic::Ordering::Relaxed);

    assert_eq!(x_value.to_string(), "65");
    assert_eq!(x_clone.to_string(), "65");
}

#[test]
#[cfg(target_pointer_width = "64")]
fn test_sizes() {
    assert_eq!(std::mem::size_of::<Value>(), 24);
}
