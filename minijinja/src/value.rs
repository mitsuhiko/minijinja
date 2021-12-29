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
//! is performed by the [`FunctionArgs`] trait.
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
//! stateful values and more.

// this module is based on the content module in insta which in turn is based
// on the content module in serde::private::ser.

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt::{self, Write};
use std::sync::atomic::{self, AtomicBool, AtomicUsize};

use serde::ser::{self, Serialize, Serializer};

use crate::error::{Error, ErrorKind};
use crate::key::{Key, KeySerializer};
use crate::utils::matches;
use crate::vm::State;

#[cfg(feature = "sync")]
pub(crate) type RcType<T> = std::sync::Arc<T>;

#[cfg(not(feature = "sync"))]
pub(crate) type RcType<T> = std::rc::Rc<T>;

// We use in-band signalling to roundtrip some internal values.  This is
// not ideal but unfortunately there is no better system in serde today.
const VALUE_HANDLE_MARKER: &str = "\x01__minijinja_ValueHandle";

#[cfg(feature = "preserve_order")]
type ValueMap<K, V> = indexmap::IndexMap<K, V>;

#[cfg(not(feature = "preserve_order"))]
type ValueMap<K, V> = std::collections::BTreeMap<K, V>;

thread_local! {
    static INTERNAL_SERIALIZATION: AtomicBool = AtomicBool::new(false);
    static LAST_VALUE_HANDLE: AtomicUsize = AtomicUsize::new(0);
    static VALUE_HANDLES: RefCell<BTreeMap<usize, Value>> = RefCell::new(BTreeMap::new());
}

fn in_internal_serialization() -> bool {
    INTERNAL_SERIALIZATION.with(|flag| flag.load(atomic::Ordering::Relaxed))
}

/// Helper trait representing valid filter and test arguments.
///
/// Since it's more convenient to write filters and tests with concrete
/// types instead of values, this helper trait exists to automatically
/// perform this conversion.  It is implemented for functions up to an
/// arity of 5 parameters.
///
/// For each argument the conversion is performed via the [`ArgType`]
/// trait which is implemented for some primitive concrete types as well
/// as these types wrapped in [`Option`].
pub trait FunctionArgs: Sized {
    /// Converts to function arguments from a slice of values.
    fn from_values(values: Vec<Value>) -> Result<Self, Error>;
}

/// A trait implemented by all filter/test argument types.
///
/// This trait is the companion to [`FunctionArgs`].  It's passed an
/// `Option<Value>` where `Some` means the argument was provided or
/// `None` if it was not.  This is used to implement optional arguments
/// to functions.
pub trait ArgType: Sized {
    fn from_value(value: Option<Value>) -> Result<Self, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<$($name: ArgType,)*> FunctionArgs for ($($name,)*) {
            fn from_values(values: Vec<Value>) -> Result<Self, Error> {
                #![allow(non_snake_case, unused)]
                let arg_count = 0 $(
                    + { let $name = (); 1 }
                )*;
                if values.len() > arg_count {
                    return Err(Error::new(
                        ErrorKind::InvalidArguments,
                        "received unexpected extra arguments",
                    ));
                }
                {
                    let mut idx = 0;
                    $(
                        let $name = ArgType::from_value(values.get(idx).cloned())?;
                        idx += 1;
                    )*
                    Ok(( $($name,)* ))
                }
            }
        }
    };
}

tuple_impls! {}
tuple_impls! { A }
tuple_impls! { A B }
tuple_impls! { A B C }
tuple_impls! { A B C D }
tuple_impls! { A B C D E }

/// Describes the kind of value.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ValueKind {
    Undefined,
    None,
    Bool,
    Number,
    Char,
    String,
    Bytes,
    Seq,
    Map,
    Struct,
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
            ValueKind::Struct => "struct",
        };
        write!(f, "{}", ty)
    }
}

#[derive(Clone)]
enum Repr {
    Undefined,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    Char(char),
    None,
    U128(RcType<u128>),
    I128(RcType<i128>),
    String(RcType<String>),
    SafeString(RcType<String>),
    Bytes(RcType<Vec<u8>>),
    Seq(RcType<Vec<Value>>),
    Map(RcType<ValueMap<Key<'static>, Value>>),
    Struct(RcType<ValueMap<&'static str, Value>>),
    Dynamic(RcType<dyn Object>),
}

impl fmt::Debug for Repr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Repr::Undefined => write!(f, "Undefined"),
            Repr::Bool(val) => fmt::Debug::fmt(val, f),
            Repr::U64(val) => fmt::Debug::fmt(val, f),
            Repr::I64(val) => fmt::Debug::fmt(val, f),
            Repr::F64(val) => fmt::Debug::fmt(val, f),
            Repr::Char(val) => fmt::Debug::fmt(val, f),
            Repr::None => write!(f, "None"),
            Repr::U128(val) => fmt::Debug::fmt(val, f),
            Repr::I128(val) => fmt::Debug::fmt(val, f),
            Repr::String(val) => fmt::Debug::fmt(val, f),
            Repr::SafeString(val) => fmt::Debug::fmt(val, f),
            Repr::Bytes(val) => fmt::Debug::fmt(val, f),
            Repr::Seq(val) => fmt::Debug::fmt(val, f),
            Repr::Map(val) => fmt::Debug::fmt(val, f),
            Repr::Struct(val) => {
                let mut s = f.debug_struct("Struct");
                for (k, v) in val.iter() {
                    s.field(k, v);
                }
                s.finish()
            }
            Repr::Dynamic(val) => fmt::Debug::fmt(val, f),
        }
    }
}

/// Represents a dynamically typed value in the template engine.
#[derive(Clone)]
pub struct Value(Repr);

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_primitive(), other.as_primitive()) {
            (Some(Primitive::None), Some(Primitive::None)) => true,
            (Some(Primitive::Str(a)), Some(Primitive::Str(b))) => a == b,
            (Some(Primitive::Bytes(a)), Some(Primitive::Bytes(b))) => a == b,
            (Some(a), Some(b)) => match coerce(a, b) {
                Some(CoerceResult::F64(a, b)) => a == b,
                Some(CoerceResult::I128(a, b)) => a == b,
                None => false,
            },
            _ => false,
        }
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.as_primitive(), other.as_primitive()) {
            (Some(Primitive::None), Some(Primitive::None)) => Some(Ordering::Equal),
            (Some(Primitive::Str(a)), Some(Primitive::Str(b))) => a.partial_cmp(b),
            (Some(Primitive::Bytes(a)), Some(Primitive::Bytes(b))) => a.partial_cmp(b),
            (Some(a), Some(b)) => match coerce(a, b) {
                Some(CoerceResult::F64(a, b)) => a.partial_cmp(&b),
                Some(CoerceResult::I128(a, b)) => a.partial_cmp(&b),
                None => None,
            },
            _ => None,
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl From<Repr> for Value {
    #[inline(always)]
    fn from(val: Repr) -> Value {
        Value(val)
    }
}

impl<'a> From<&'a [u8]> for Value {
    #[inline(always)]
    fn from(val: &'a [u8]) -> Self {
        Repr::Bytes(RcType::new(val.into())).into()
    }
}

impl<'a> From<&'a str> for Value {
    #[inline(always)]
    fn from(val: &'a str) -> Self {
        Repr::String(RcType::new(val.into())).into()
    }
}

impl From<String> for Value {
    #[inline(always)]
    fn from(val: String) -> Self {
        Repr::String(RcType::new(val)).into()
    }
}

impl<'a> From<Cow<'a, str>> for Value {
    #[inline(always)]
    fn from(val: Cow<'a, str>) -> Self {
        match val {
            Cow::Borrowed(x) => x.into(),
            Cow::Owned(x) => x.into(),
        }
    }
}

impl From<()> for Value {
    #[inline(always)]
    fn from(_: ()) -> Self {
        Repr::None.into()
    }
}

impl From<i128> for Value {
    #[inline(always)]
    fn from(val: i128) -> Self {
        Repr::I128(RcType::new(val)).into()
    }
}

impl From<u128> for Value {
    #[inline(always)]
    fn from(val: u128) -> Self {
        Repr::U128(RcType::new(val)).into()
    }
}

impl<'a> From<Key<'a>> for Value {
    fn from(val: Key) -> Self {
        match val {
            Key::Bool(val) => val.into(),
            Key::I64(val) => val.into(),
            Key::Char(val) => val.into(),
            Key::String(val) => Repr::String(val).into(),
            Key::Str(val) => val.into(),
        }
    }
}

impl<K: Into<Key<'static>>, V: Into<Value>> From<BTreeMap<K, V>> for Value {
    fn from(val: BTreeMap<K, V>) -> Self {
        Repr::Map(RcType::new(
            val.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        ))
        .into()
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(val: Vec<T>) -> Self {
        Repr::Seq(RcType::new(val.into_iter().map(|x| x.into()).collect())).into()
    }
}

macro_rules! value_from {
    ($src:ty, $dst:ident) => {
        impl From<$src> for Value {
            #[inline(always)]
            fn from(val: $src) -> Self {
                Repr::$dst(val as _).into()
            }
        }
    };
}

value_from!(bool, Bool);
value_from!(u8, U64);
value_from!(u16, U64);
value_from!(u32, U64);
value_from!(u64, U64);
value_from!(i8, I64);
value_from!(i16, I64);
value_from!(i32, I64);
value_from!(i64, I64);
value_from!(f32, F64);
value_from!(f64, F64);
value_from!(char, Char);

fn format_seqish<I: Iterator<Item = D>, D: fmt::Display>(
    f: &mut fmt::Formatter<'_>,
    iter: I,
) -> fmt::Result {
    for (idx, val) in iter.enumerate() {
        if idx > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{}", val)?;
    }
    Ok(())
}

/// An alternative view of a value.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Primitive<'a> {
    Undefined,
    None,
    Bool(bool),
    U64(u64),
    U128(u128),
    I64(i64),
    I128(i128),
    F64(f64),
    Char(char),
    Str(&'a str),
    Bytes(&'a [u8]),
}

impl<'a> Primitive<'a> {
    pub fn as_f64(self) -> Option<f64> {
        Some(match self {
            Primitive::Bool(true) => 1.0,
            Primitive::Bool(false) => 0.0,
            Primitive::Char(x) => x as i64 as f64,
            Primitive::U64(x) => x as f64,
            Primitive::U128(x) => x as f64,
            Primitive::I64(x) => x as f64,
            Primitive::I128(x) => x as f64,
            Primitive::F64(x) => x,
            _ => return None,
        })
    }

    pub fn as_i128(self) -> Option<i128> {
        Some(match self {
            Primitive::Bool(true) => 1,
            Primitive::Bool(false) => 0,
            Primitive::Char(x) => x as i128,
            Primitive::U64(x) => x as i128,
            Primitive::U128(x) => x as i128,
            Primitive::I64(x) => x as i128,
            Primitive::I128(x) => x as i128,
            Primitive::F64(x) => x as i128,
            _ => return None,
        })
    }
}

enum CoerceResult {
    I128(i128, i128),
    F64(f64, f64),
}

fn coerce<'a>(a: Primitive<'a>, b: Primitive<'a>) -> Option<CoerceResult> {
    match (a, b) {
        // equal mappings are trivial
        (Primitive::U64(a), Primitive::U64(b)) => Some(CoerceResult::I128(a as i128, b as i128)),
        (Primitive::U128(a), Primitive::U128(b)) => Some(CoerceResult::I128(a as i128, b as i128)),
        (Primitive::I64(a), Primitive::I64(b)) => Some(CoerceResult::I128(a as i128, b as i128)),
        (Primitive::I128(a), Primitive::I128(b)) => Some(CoerceResult::I128(a, b)),
        (Primitive::F64(a), Primitive::F64(b)) => Some(CoerceResult::F64(a, b)),

        // are floats involved?
        (Primitive::F64(a), _) => Some(CoerceResult::F64(a, b.as_f64()?)),
        (_, Primitive::F64(b)) => Some(CoerceResult::F64(a.as_f64()?, b)),

        // everything else goes up to i128
        (_, _) => Some(CoerceResult::I128(a.as_i128()?, b.as_i128()?)),
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Repr::Undefined => Ok(()),
            Repr::Bool(val) => write!(f, "{}", val),
            Repr::U64(val) => write!(f, "{}", val),
            Repr::I64(val) => write!(f, "{}", val),
            Repr::F64(val) => write!(f, "{}", val),
            Repr::Char(val) => write!(f, "{}", val),
            Repr::None => write!(f, "none"),
            Repr::I128(val) => write!(f, "{}", val),
            Repr::String(val) => write!(f, "{}", val),
            Repr::SafeString(val) => write!(f, "{}", val),
            Repr::Bytes(val) => write!(f, "{}", String::from_utf8_lossy(val)),
            Repr::Seq(values) => format_seqish(f, values.iter()),
            Repr::Map(val) => format_seqish(f, val.iter().map(|x| x.0)),
            Repr::Struct(val) => {
                for (idx, (key, _)) in val.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", key)?;
                }
                Ok(())
            }
            Repr::U128(val) => write!(f, "{}", val),
            Repr::Dynamic(x) => write!(f, "{}", x),
        }
    }
}

impl Default for Value {
    fn default() -> Value {
        Repr::None.into()
    }
}

fn int_as_value(val: i128) -> Value {
    if val as i64 as i128 == val {
        (val as i64).into()
    } else {
        val.into()
    }
}

macro_rules! math_binop {
    ($name:ident, $int:ident, $float:tt) => {
        pub(crate) fn $name(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
            pub fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
                match coerce(lhs.as_primitive()?, rhs.as_primitive()?)? {
                    CoerceResult::I128(a, b) => Some(int_as_value(a.$int(b))),
                    CoerceResult::F64(a, b) => Some((a $float b).into()),
                }
            }
            do_it(lhs, rhs).ok_or_else(|| {
                Error::new(
                    ErrorKind::ImpossibleOperation,
                    format!(
                        "tried to use {} operator on unsupported types {} and {}",
                        stringify!($float),
                        lhs.kind(),
                        rhs.kind()
                    )
                )
            })
        }
    }
}

math_binop!(add, wrapping_add, +);
math_binop!(sub, wrapping_sub, -);
math_binop!(mul, wrapping_mul, *);
math_binop!(div, wrapping_div, /);
math_binop!(rem, wrapping_rem_euclid, %);

/// Implements a binary `pow` operation on values.
pub(crate) fn pow(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    pub fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
        match coerce(lhs.as_primitive()?, rhs.as_primitive()?)? {
            CoerceResult::I128(a, b) => Some(int_as_value(a.pow(TryFrom::try_from(b).ok()?))),
            CoerceResult::F64(a, b) => Some((a.powf(b)).into()),
        }
    }
    do_it(lhs, rhs).ok_or_else(|| {
        Error::new(
            ErrorKind::ImpossibleOperation,
            concat!("could not calculate the power"),
        )
    })
}

/// Implements an unary `neg` operation on value.
pub(crate) fn neg(val: &Value) -> Result<Value, Error> {
    fn do_it(val: &Value) -> Option<Value> {
        let val = val.as_primitive()?;
        match val {
            Primitive::F64(_) => Some((-val.as_f64()?).into()),
            _ => Some(int_as_value(-val.as_i128()?)),
        }
    }

    do_it(val).ok_or_else(|| Error::from(ErrorKind::ImpossibleOperation))
}

/// Attempts a string concatenation.
pub(crate) fn string_concat(mut left: Value, right: &Value) -> Value {
    match left.0 {
        // if we're a string and we have a single reference to it, we can
        // directly append into ourselves and reconstruct the value
        Repr::String(ref mut s) => {
            write!(RcType::make_mut(s), "{}", right).ok();
            left
        }
        // otherwise we use format! to concat the two values
        _ => Value::from(format!("{}{}", left, right)),
    }
}

/// Implements a containment operation on values.
pub(crate) fn contains(container: &Value, value: &Value) -> Result<Value, Error> {
    match container.0 {
        Repr::Seq(ref values) => Ok(Value::from(values.contains(value))),
        Repr::Map(ref map) => {
            let key = match value.clone().try_into_key() {
                Ok(key) => key,
                Err(_) => return Ok(Value::from(false)),
            };
            return Ok(Value::from(map.get(&key).is_some()));
        }
        Repr::String(ref s) | Repr::SafeString(ref s) => {
            return Ok(Value::from(if let Some(s2) = value.as_str() {
                s.contains(&s2)
            } else {
                s.contains(&value.to_string())
            }));
        }
        _ => Err(Error::new(
            ErrorKind::ImpossibleOperation,
            "cannot perform a containment check on this value",
        )),
    }
}

macro_rules! primitive_try_from {
    ($ty:ident, {
        $($pat:pat => $expr:expr,)*
    }) => {

        impl TryFrom<Value> for $ty {
            type Error = Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                let opt = match value.as_primitive() {
                    $(Some($pat) => TryFrom::try_from($expr).ok(),)*
                    _ => None
                };
                opt.ok_or_else(|| {
                    Error::new(ErrorKind::ImpossibleOperation, concat!("cannot convert to ", stringify!($ty)))
                })
            }
        }

        impl ArgType for $ty {
            fn from_value(value: Option<Value>) -> Result<Self, Error> {
                match value {
                    Some(value) => TryFrom::try_from(value.clone()),
                    None => Err(Error::new(ErrorKind::UndefinedError, concat!("missing argument")))
                }
            }
        }

        impl ArgType for Option<$ty> {
            fn from_value(value: Option<Value>) -> Result<Self, Error> {
                match value {
                    Some(value) => {
                        if value.is_undefined() || value.is_none() {
                            Ok(None)
                        } else {
                            TryFrom::try_from(value.clone()).map(Some)
                        }
                    }
                    None => Ok(None),
                }
            }
        }
    }
}

macro_rules! primitive_int_try_from {
    ($ty:ident) => {
        primitive_try_from!($ty, {
            Primitive::I64(val) => val,
            Primitive::U64(val) => val,
        });
    }
}

primitive_int_try_from!(u8);
primitive_int_try_from!(u16);
primitive_int_try_from!(u32);
primitive_int_try_from!(u64);
primitive_int_try_from!(u128);
primitive_int_try_from!(i8);
primitive_int_try_from!(i16);
primitive_int_try_from!(i32);
primitive_int_try_from!(i64);
primitive_int_try_from!(i128);
primitive_int_try_from!(usize);

primitive_try_from!(bool, {
    Primitive::Bool(val) => val,
});

primitive_try_from!(f64, {
    Primitive::F64(val) => val,
});

macro_rules! infallible_conversion {
    ($ty:ty) => {
        impl ArgType for $ty {
            fn from_value(value: Option<Value>) -> Result<Self, Error> {
                match value {
                    Some(value) => Ok(value.clone().into()),
                    None => Err(Error::new(
                        ErrorKind::UndefinedError,
                        concat!("missing argument"),
                    )),
                }
            }
        }

        impl ArgType for Option<$ty> {
            fn from_value(value: Option<Value>) -> Result<Self, Error> {
                match value {
                    Some(value) => {
                        if value.is_undefined() || value.is_none() {
                            Ok(None)
                        } else {
                            Ok(Some(value.clone().into()))
                        }
                    }
                    None => Ok(None),
                }
            }
        }
    };
}

infallible_conversion!(String);
infallible_conversion!(Value);

impl From<Value> for String {
    fn from(val: Value) -> Self {
        val.to_string()
    }
}

impl From<usize> for Value {
    fn from(val: usize) -> Self {
        Value::from(val as u64)
    }
}

impl<T: ArgType> ArgType for Vec<T> {
    fn from_value(value: Option<Value>) -> Result<Self, Error> {
        match value {
            None => Ok(Vec::new()),
            Some(values) => {
                let values = values.try_into_vec()?;
                let mut rv = Vec::new();
                for value in values {
                    rv.push(ArgType::from_value(Some(value))?);
                }
                Ok(rv)
            }
        }
    }
}

#[allow(clippy::len_without_is_empty)]
impl Value {
    /// The undefined value
    pub const UNDEFINED: Value = Value(Repr::Undefined);

    /// Creates a value from something that can be serialized.
    pub fn from_serializable<T: Serialize>(value: &T) -> Value {
        INTERNAL_SERIALIZATION.with(|flag| {
            let old = flag.load(atomic::Ordering::Relaxed);
            flag.store(true, atomic::Ordering::Relaxed);
            let rv = Serialize::serialize(value, ValueSerializer);
            flag.store(old, atomic::Ordering::Relaxed);
            rv.unwrap()
        })
    }

    /// Creates a value from a safe string.
    pub fn from_safe_string(value: String) -> Value {
        Repr::SafeString(RcType::new(value)).into()
    }

    /// Creates a value from a reference counted dynamic object.
    pub(crate) fn from_rc_object<T: Object + 'static>(value: RcType<T>) -> Value {
        Repr::Dynamic(value as RcType<dyn Object>).into()
    }

    /// Creates a value from a dynamic object.
    pub fn from_object<T: Object + 'static>(value: T) -> Value {
        Value::from_rc_object(RcType::new(value))
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
        if let Repr::Dynamic(ref obj) = self.0 {
            if (**obj).type_id() == TypeId::of::<T>() {
                unsafe {
                    // newer versions of Rust have RcType::as_ptr but we support
                    // rust versions down to 1.41.0 so we need to use a workaround here.
                    let count = RcType::strong_count(obj);
                    let clone = obj.clone();
                    let raw: *const (dyn Object) = RcType::into_raw(clone);
                    let rv = (raw as *const u8 as *const T).as_ref();
                    RcType::from_raw(raw);
                    debug_assert_eq!(count, RcType::strong_count(obj));
                    return rv;
                }
            }
        }
        None
    }

    /// Returns the value kind.
    pub fn kind(&self) -> ValueKind {
        match self.0 {
            Repr::Undefined => ValueKind::Undefined,
            Repr::Bool(_) => ValueKind::Bool,
            Repr::U64(_) | Repr::I64(_) | Repr::F64(_) => ValueKind::Number,
            Repr::Char(_) => ValueKind::Char,
            Repr::None => ValueKind::None,
            Repr::I128(_) => ValueKind::Number,
            Repr::String(_) | Repr::SafeString(_) => ValueKind::String,
            Repr::Bytes(_) => ValueKind::Bytes,
            Repr::U128(_) => ValueKind::Number,
            Repr::Seq(_) => ValueKind::Seq,
            Repr::Map(_) => ValueKind::Map,
            Repr::Struct(_) | Repr::Dynamic(_) => ValueKind::Struct,
        }
    }

    /// Returns the primitive representation of the value.
    pub fn as_primitive(&self) -> Option<Primitive<'_>> {
        match self.0 {
            Repr::Undefined => Some(Primitive::Undefined),
            Repr::Bool(val) => Some(Primitive::Bool(val)),
            Repr::U64(val) => Some(Primitive::U64(val)),
            Repr::I64(val) => Some(Primitive::I64(val)),
            Repr::F64(val) => Some(Primitive::F64(val)),
            Repr::Char(val) => Some(Primitive::Char(val)),
            Repr::None => Some(Primitive::None),
            Repr::I128(ref val) => Some(Primitive::I128(**val)),
            Repr::String(ref val) => Some(Primitive::Str(val.as_str())),
            Repr::SafeString(ref val) => Some(Primitive::Str(val.as_str())),
            Repr::Bytes(ref val) => Some(Primitive::Bytes(&val[..])),
            Repr::U128(ref val) => Some(Primitive::U128(**val)),
            _ => None,
        }
    }

    /// If the value is a string, return it.
    pub fn as_str(&self) -> Option<&str> {
        match self.as_primitive() {
            Some(Primitive::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// Is this value true?
    pub fn is_true(&self) -> bool {
        match self.as_primitive() {
            Some(Primitive::Bool(val)) => val,
            Some(Primitive::U64(x)) => x != 0,
            Some(Primitive::U128(x)) => x != 0,
            Some(Primitive::I64(x)) => x != 0,
            Some(Primitive::I128(x)) => x != 0,
            Some(Primitive::F64(x)) => x != 0.0,
            Some(Primitive::Char(x)) => x != '\x00',
            Some(Primitive::Str(x)) => !x.is_empty(),
            Some(Primitive::Bytes(x)) => !x.is_empty(),
            Some(Primitive::None) | Some(Primitive::Undefined) => false,
            None => true,
        }
    }

    /// Returns `true` if this value is safe.
    pub fn is_safe(&self) -> bool {
        matches!(&self.0, Repr::SafeString(_))
    }

    /// Returns `true` if this value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(&self.0, Repr::Undefined)
    }

    /// Returns `true` if this value is none.
    pub fn is_none(&self) -> bool {
        matches!(&self.0, Repr::None)
    }

    /// Returns the length of the contained value.
    pub fn len(&self) -> Option<usize> {
        match self.0 {
            Repr::String(ref s) | Repr::SafeString(ref s) => Some(s.chars().count()),
            Repr::Map(ref items) => Some(items.len()),
            Repr::Struct(ref items) => Some(items.len()),
            Repr::Seq(ref items) => Some(items.len()),
            Repr::Dynamic(ref dy) => Some(dy.attributes().len()),
            _ => None,
        }
    }

    /// Looks up an attribute by attribute name.
    pub fn get_attr(&self, key: &str) -> Result<Value, Error> {
        let value = match self.0 {
            Repr::Map(ref items) => {
                let lookup_key = Key::Str(key);
                items.get(&lookup_key).cloned()
            }
            Repr::Struct(ref items) => items.get(key).cloned(),
            Repr::Dynamic(ref dy) => dy.get_attr(key),
            Repr::Undefined => {
                return Err(Error::from(ErrorKind::UndefinedError));
            }
            _ => None,
        };
        Ok(value.unwrap_or(Value::UNDEFINED))
    }

    /// Looks up an item (or attribute) by key.
    ///
    /// This is similar to [`get_attr`](Value::get_attr) but instead of using
    /// a string key this can be any key.  For instance this can be used to
    /// index into sequences.
    pub fn get_item(&self, key: &Value) -> Result<Value, Error> {
        if let Repr::Undefined = self.0 {
            Err(Error::from(ErrorKind::UndefinedError))
        } else {
            Ok(self.get_item_opt(key).unwrap_or(Value::UNDEFINED))
        }
    }

    fn get_item_opt(&self, key: &Value) -> Option<Value> {
        let key = Key::from_borrowed_value(key).ok()?;

        match self.0 {
            Repr::Map(ref items) => return items.get(&key).cloned(),
            Repr::Struct(ref items) => {
                if let Key::String(ref key) = key {
                    return items.get(key.as_str()).cloned();
                }
            }
            Repr::Seq(ref items) => {
                if let Key::I64(idx) = key {
                    let idx = isize::try_from(idx).ok()?;
                    let idx = if idx < 0 {
                        items.len() - (-idx as usize)
                    } else {
                        idx as usize
                    };
                    return items.get(idx).cloned();
                }
            }
            Repr::Dynamic(ref dy) => match key {
                Key::String(ref key) => return dy.get_attr(key),
                Key::Str(key) => return dy.get_attr(key),
                _ => {}
            },
            _ => {}
        }
        None
    }

    /// Calls the value directly.
    pub(crate) fn call(&self, state: &State, args: Vec<Value>) -> Result<Value, Error> {
        if let Repr::Dynamic(ref dy) = self.0 {
            dy.call(state, args)
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                format!("value of type {} is not callable", self.kind()),
            ))
        }
    }

    /// Calls a method on the value.
    pub(crate) fn call_method(
        &self,
        state: &State,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, Error> {
        if let Repr::Dynamic(ref dy) = self.0 {
            dy.call_method(state, name, args)
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                format!("object has no method named {}", name),
            ))
        }
    }

    pub(crate) fn try_into_key(self) -> Result<Key<'static>, Error> {
        match self.0 {
            Repr::Bool(val) => Ok(Key::Bool(val)),
            Repr::U64(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Repr::U128(ref v) => TryFrom::try_from(**v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Repr::I64(v) => Ok(Key::I64(v)),
            Repr::I128(ref v) => TryFrom::try_from(**v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Repr::Char(c) => Ok(Key::Char(c)),
            Repr::String(ref s) => Ok(Key::String(s.clone())),
            _ => Err(ErrorKind::NonKey.into()),
        }
    }

    pub(crate) fn try_into_vec(self) -> Result<Vec<Value>, Error> {
        match self.0 {
            Repr::Seq(v) => Ok(match RcType::try_unwrap(v) {
                Ok(v) => v,
                Err(rc) => (*rc).clone(),
            }),
            _ => Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot convert value into list",
            )),
        }
    }

    #[cfg(feature = "builtin_filters")]
    pub(crate) fn try_into_pairs(self) -> Result<Vec<(Value, Value)>, Error> {
        match self.0 {
            Repr::Map(v) => Ok(match RcType::try_unwrap(v) {
                Ok(v) => v,
                Err(rc) => (*rc).clone(),
            }
            .into_iter()
            .map(|(k, v)| (Value::from(k), v))
            .collect()),
            _ => Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot convert value into pair list",
            )),
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn iter_as_str_map(&self) -> impl Iterator<Item = (&str, Value)> {
        match self.0 {
            Repr::Map(ref m) => Box::new(
                m.iter()
                    .filter_map(|(k, v)| k.as_str().map(move |k| (k, v.clone()))),
            ) as Box<dyn Iterator<Item = _>>,
            Repr::Struct(ref s) => {
                Box::new(s.iter().map(|(k, v)| (*k, v.clone()))) as Box<dyn Iterator<Item = _>>
            }
            Repr::Dynamic(ref obj) => Box::new(
                obj.attributes()
                    .iter()
                    .filter_map(move |attr| Some((*attr, obj.get_attr(attr)?))),
            ) as Box<dyn Iterator<Item = _>>,
            _ => Box::new(None.into_iter()) as Box<dyn Iterator<Item = _>>,
        }
    }

    /// Iterates over the value.
    pub(crate) fn iter(&self) -> ValueIterator {
        let (iter_state, len) = match self.0 {
            Repr::Seq(ref seq) => (ValueIteratorState::Seq(0, RcType::clone(seq)), seq.len()),
            #[cfg(feature = "preserve_order")]
            Repr::Map(ref items) => (
                ValueIteratorState::Map(0, RcType::clone(items)),
                items.len(),
            ),
            #[cfg(not(feature = "preserve_order"))]
            Repr::Map(ref items) => (
                ValueIteratorState::Map(
                    items.iter().next().map(|x| x.0.clone()),
                    RcType::clone(items),
                ),
                items.len(),
            ),
            #[cfg(feature = "preserve_order")]
            Repr::Struct(ref fields) => (
                ValueIteratorState::Struct(0, RcType::clone(fields)),
                fields.len(),
            ),
            #[cfg(not(feature = "preserve_order"))]
            Repr::Struct(ref fields) => (
                ValueIteratorState::Struct(
                    fields.iter().next().map(|x| *x.0),
                    RcType::clone(fields),
                ),
                fields.len(),
            ),
            _ => (ValueIteratorState::Empty, 0),
        };
        ValueIterator { iter_state, len }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // enable round tripping of values
        if in_internal_serialization() {
            use serde::ser::SerializeStruct;
            let handle = LAST_VALUE_HANDLE.with(|x| x.fetch_add(1, atomic::Ordering::Relaxed));
            VALUE_HANDLES.with(|handles| handles.borrow_mut().insert(handle, self.clone()));
            let mut s = serializer.serialize_struct(VALUE_HANDLE_MARKER, 1)?;
            s.serialize_field("handle", &handle)?;
            return s.end();
        }

        match self.0 {
            Repr::Bool(b) => serializer.serialize_bool(b),
            Repr::U64(u) => serializer.serialize_u64(u),
            Repr::I64(i) => serializer.serialize_i64(i),
            Repr::F64(f) => serializer.serialize_f64(f),
            Repr::Char(c) => serializer.serialize_char(c),
            Repr::None => serializer.serialize_unit(),
            Repr::Undefined => serializer.serialize_unit(),
            Repr::U128(ref u) => serializer.serialize_u128(**u),
            Repr::I128(ref i) => serializer.serialize_i128(**i),
            Repr::String(ref s) => serializer.serialize_str(s),
            Repr::SafeString(ref val) => serializer.serialize_str(val),
            Repr::Bytes(ref b) => serializer.serialize_bytes(b),
            Repr::Seq(ref elements) => elements.serialize(serializer),
            Repr::Map(ref entries) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (ref k, ref v) in entries.iter() {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            Repr::Struct(ref fields) => {
                use serde::ser::SerializeStruct;
                let mut s = serializer.serialize_struct("Struct", fields.len())?;
                for (k, ref v) in fields.iter() {
                    s.serialize_field(k, v)?;
                }
                s.end()
            }
            Repr::Dynamic(ref n) => {
                use serde::ser::SerializeMap;
                let fields = n.attributes();
                let mut s = serializer.serialize_map(Some(fields.len()))?;
                for k in fields {
                    let v = n.get_attr(k).unwrap_or(Value::UNDEFINED);
                    s.serialize_entry(k, &v)?;
                }
                s.end()
            }
        }
    }
}

struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;

    type SerializeSeq = SerializeSeq;
    type SerializeTuple = SerializeTuple;
    type SerializeTupleStruct = SerializeTupleStruct;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeStruct;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Value, Error> {
        Ok(Repr::Bool(v).into())
    }

    fn serialize_i8(self, v: i8) -> Result<Value, Error> {
        Ok(Repr::I64(v as i64).into())
    }

    fn serialize_i16(self, v: i16) -> Result<Value, Error> {
        Ok(Repr::I64(v as i64).into())
    }

    fn serialize_i32(self, v: i32) -> Result<Value, Error> {
        Ok(Repr::I64(v as i64).into())
    }

    fn serialize_i64(self, v: i64) -> Result<Value, Error> {
        Ok(Repr::I64(v).into())
    }

    fn serialize_i128(self, v: i128) -> Result<Value, Error> {
        Ok(Repr::I128(RcType::new(v)).into())
    }

    fn serialize_u8(self, v: u8) -> Result<Value, Error> {
        Ok(Repr::U64(v as u64).into())
    }

    fn serialize_u16(self, v: u16) -> Result<Value, Error> {
        Ok(Repr::U64(v as u64).into())
    }

    fn serialize_u32(self, v: u32) -> Result<Value, Error> {
        Ok(Repr::U64(v as u64).into())
    }

    fn serialize_u64(self, v: u64) -> Result<Value, Error> {
        Ok(Repr::U64(v).into())
    }

    fn serialize_u128(self, v: u128) -> Result<Value, Error> {
        Ok(Repr::U128(RcType::new(v)).into())
    }

    fn serialize_f32(self, v: f32) -> Result<Value, Error> {
        Ok(Repr::F64(v as f64).into())
    }

    fn serialize_f64(self, v: f64) -> Result<Value, Error> {
        Ok(Repr::F64(v).into())
    }

    fn serialize_char(self, v: char) -> Result<Value, Error> {
        Ok(Repr::Char(v).into())
    }

    fn serialize_str(self, value: &str) -> Result<Value, Error> {
        Ok(Repr::String(RcType::new(value.to_owned())).into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, Error> {
        Ok(Repr::Bytes(RcType::new(value.to_owned())).into())
    }

    fn serialize_none(self) -> Result<Value, Error> {
        Ok(Repr::None.into())
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Value, Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(Repr::None.into())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(Repr::None.into())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(Repr::String(RcType::new(variant.to_string())).into())
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Error>
    where
        T: Serialize,
    {
        let mut map = ValueMap::new();
        map.insert(Key::from(variant), value.serialize(self)?);
        Ok(Repr::Map(RcType::new(map)).into())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Ok(SerializeSeq {
            elements: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Error> {
        Ok(SerializeTuple {
            elements: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Ok(SerializeTupleStruct {
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Ok(SerializeTupleVariant {
            name: variant,
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(SerializeMap {
            entries: ValueMap::new(),
            key: None,
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Ok(SerializeStruct {
            name,
            fields: ValueMap::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Ok(SerializeStructVariant {
            variant,
            map: ValueMap::new(),
        })
    }
}

struct SerializeSeq {
    elements: Vec<Value>,
}

impl ser::SerializeSeq for SerializeSeq {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.elements.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Repr::Seq(RcType::new(self.elements)).into())
    }
}

struct SerializeTuple {
    elements: Vec<Value>,
}

impl ser::SerializeTuple for SerializeTuple {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.elements.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Repr::Seq(RcType::new(self.elements)).into())
    }
}

struct SerializeTupleStruct {
    fields: Vec<Value>,
}

impl ser::SerializeTupleStruct for SerializeTupleStruct {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.fields.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value(Repr::Seq(RcType::new(self.fields))))
    }
}

struct SerializeTupleVariant {
    name: &'static str,
    fields: Vec<Value>,
}

impl ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.fields.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        let mut map = BTreeMap::new();
        map.insert(self.name, self.fields);
        Ok(map.into())
    }
}

struct SerializeMap {
    entries: ValueMap<Key<'static>, Value>,
    key: Option<Key<'static>>,
}

impl ser::SerializeMap for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let key = key.serialize(KeySerializer)?;
        self.key = Some(key);
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let key = self
            .key
            .take()
            .expect("serialize_value called before serialize_key");
        let value = value.serialize(ValueSerializer)?;
        self.entries.insert(key, value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value(Repr::Map(RcType::new(self.entries))))
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(&mut self, key: &K, value: &V) -> Result<(), Error>
    where
        K: Serialize,
        V: Serialize,
    {
        let key = key.serialize(KeySerializer)?;
        let value = value.serialize(ValueSerializer)?;
        self.entries.insert(key, value);
        Ok(())
    }
}

struct SerializeStruct {
    name: &'static str,
    fields: ValueMap<&'static str, Value>,
}

impl ser::SerializeStruct for SerializeStruct {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.fields.insert(key, value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        match self.name {
            VALUE_HANDLE_MARKER => {
                let handle_id = match self.fields.get("handle") {
                    Some(&Value(Repr::U64(handle_id))) => handle_id as usize,
                    _ => panic!("bad handle reference in value roundtrip"),
                };
                Ok(VALUE_HANDLES.with(|handles| {
                    let mut handles = handles.borrow_mut();
                    handles
                        .remove(&handle_id)
                        .expect("value handle not in registry")
                }))
            }
            _ => Ok(Repr::Struct(RcType::new(self.fields)).into()),
        }
    }
}

struct SerializeStructVariant {
    variant: &'static str,
    map: ValueMap<Key<'static>, Value>,
}

impl ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.map.insert(Key::from(key), value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        let mut rv = BTreeMap::new();
        rv.insert(self.variant, Value::from(Repr::Map(RcType::new(self.map))));
        Ok(rv.into())
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

impl<'a> fmt::Debug for ValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueIterator").finish()
    }
}

/// Helper to pass a single value as context bound to a name.
///
/// The first argument is the name of the variable, the second is the
/// value to pass.  Here a simple motivating example:
///
/// ```
/// use minijinja::{Environment, context};
///
/// let mut env = Environment::new();
/// env.add_template("simple", "Hello {{ name }}!").unwrap();
/// let tmpl = env.get_template("simple").unwrap();
/// let rv = tmpl.render(context!(name => "Peter")).unwrap();
/// ```
///
/// This type is deprecated and has been replaced with the [`context`] macro.
#[derive(Debug, Clone)]
#[deprecated(since = "0.8.0", note = "replaced by the more generic context! macro")]
pub struct Single<'a, V: Serialize + ?Sized>(pub &'a str, pub V);

#[allow(deprecated)]
impl<'a, V: Serialize + ?Sized> Serialize for Single<'a, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(self.0, &self.1)?;
        map.end()
    }
}

enum ValueIteratorState {
    Empty,
    Seq(usize, RcType<Vec<Value>>),
    #[cfg(not(feature = "preserve_order"))]
    Map(Option<Key<'static>>, RcType<ValueMap<Key<'static>, Value>>),
    #[cfg(feature = "preserve_order")]
    Map(usize, RcType<ValueMap<Key<'static>, Value>>),
    #[cfg(not(feature = "preserve_order"))]
    Struct(Option<&'static str>, RcType<ValueMap<&'static str, Value>>),
    #[cfg(feature = "preserve_order")]
    Struct(usize, RcType<ValueMap<&'static str, Value>>),
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
            #[cfg(feature = "preserve_order")]
            ValueIteratorState::Struct(idx, map) => map.get_index(*idx).map(|x| {
                *idx += 1;
                Value::from(*x.0)
            }),
            #[cfg(not(feature = "preserve_order"))]
            ValueIteratorState::Struct(ptr, map) => {
                if let Some(current) = ptr {
                    let rv = Value::from(*current);
                    *ptr = map.range(*current..).nth(1).map(|x| *x.0);
                    Some(rv)
                } else {
                    None
                }
            }
        }
    }
}

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
    fn call_method(&self, state: &State, name: &str, args: Vec<Value>) -> Result<Value, Error> {
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
    fn call(&self, state: &State, args: Vec<Value>) -> Result<Value, Error> {
        let _state = state;
        let _args = args;
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            "tried to call non callable object",
        ))
    }
}

/// Utility macro to create a value from a literal
#[cfg(test)]
macro_rules! value {
    ($value:expr) => {
        Value::from_serializable(&$value)
    };
}

#[test]
fn test_adding() {
    let err = add(&value!("a"), &value!(42)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "impossible operation: tried to use + operator on unsupported types string and number"
    );

    assert_eq!(add(&value!(1), &value!(2)), Ok(value!(3)));
}

#[test]
fn test_concat() {
    assert_eq!(
        string_concat(Value::from("foo"), &Value::from(42)),
        Value::from("foo42")
    );
    assert_eq!(
        string_concat(Value::from(23), &Value::from(42)),
        Value::from("2342")
    );
}

#[test]
fn test_sort() {
    let mut v = vec![
        Value::from(100u64),
        Value::from(80u32),
        Value::from(30i16),
        Value::from(true),
        Value::from(false),
        Value::from(99i128),
        Value::from(1000f32),
    ];
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    insta::assert_debug_snapshot!(&v, @r###"
    [
        false,
        true,
        30,
        80,
        99,
        100,
        1000.0,
    ]
    "###);
}

#[test]
fn test_safe_string_roundtrip() {
    let v = Value::from_safe_string("<b>HTML</b>".into());
    let v2 = Value::from_serializable(&v);
    assert!(v.is_safe());
    assert!(v2.is_safe());
    assert_eq!(v.to_string(), v2.to_string());
}

#[test]
fn test_undefined_roundtrip() {
    let v = Value::UNDEFINED;
    let v2 = Value::from_serializable(&v);
    assert!(v.is_undefined());
    assert!(v2.is_undefined());
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

    let x = RcType::new(X(Default::default()));
    let x_value = Value::from_rc_object(x.clone());
    x.0.fetch_add(42, atomic::Ordering::Relaxed);
    let x_clone = Value::from_serializable(&x_value);
    x.0.fetch_add(23, atomic::Ordering::Relaxed);

    assert_eq!(x_value.to_string(), "65");
    assert_eq!(x_clone.to_string(), "65");
}

#[test]
fn test_string_key_lookup() {
    let mut m = BTreeMap::new();
    m.insert(Key::String(RcType::new("foo".into())), Value::from(42));
    let m = Value::from(m);
    assert_eq!(m.get_item(&Value::from("foo")).unwrap(), Value::from(42));
}

#[test]
fn test_value_serialization() {
    // make sure if we serialize to json we get regular values
    assert_eq!(serde_json::to_string(&Value::UNDEFINED).unwrap(), "null");
    assert_eq!(
        serde_json::to_string(&Value::from_safe_string("foo".to_string())).unwrap(),
        "\"foo\""
    );
}

#[test]
#[cfg(target_pointer_width = "64")]
fn test_sizes() {
    assert_eq!(std::mem::size_of::<Value>(), 24);
}
