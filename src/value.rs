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

// this module is based on the content module in insta which in turn is based
// on the content module in serde::private::ser.
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt::{self, Write};
use std::marker::PhantomData;
use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;

use serde::ser::{self, Serialize, Serializer};

use crate::error::{Error, ErrorKind};
use crate::key::{Key, KeySerializer};

// the type we want to use for refcounting
pub(crate) type RcType<T> = Arc<T>;

// We use in-band signalling to roundtrip some internal values.  This is
// not ideal but unfortunately there is no better system in serde today.
const SAFESTRING_MARKER: &str = "\x01__minijinja_SafeString";
const UNDEFINED_MARKER: &str = "\x01__minijinja_Undefined";

thread_local! {
    static INTERNAL_SERIALIZATION: AtomicBool = AtomicBool::new(false);
}

fn in_internal_serialization() -> bool {
    INTERNAL_SERIALIZATION.with(|flag| flag.load(atomic::Ordering::Relaxed))
}

/// Helper trait representing valid filter and test arguments.
///
/// Since it's more convenient to write filters and tests with concrete
/// types instead of values, this helper trait exists to automatically
/// perform this conversion.
pub trait ValueArgs: Sized {
    /// Converts to function arguments from a slice of values.
    fn from_values(values: Vec<Value>) -> Result<Self, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<$($name: TryFrom<Value>,)*> ValueArgs for ($($name,)*) {
            fn from_values(values: Vec<Value>) -> Result<Self, Error> {
                #[allow(non_snake_case)]
                match values.as_slice() {
                    &[$(ref $name,)*] => Ok((
                        $(
                            TryFrom::try_from($name.clone()).map_err(|_| {
                                Error::new(
                                    ErrorKind::ImpossibleOperation,
                                    "incompatible arguemnt type for filter",
                                )
                            })?,
                        )*
                    )),
                    _ => {
                        return Err(Error::new(
                            ErrorKind::InvalidFilterArguments,
                            "invalid argument count to filter",
                        ))
                    }
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

impl ValueArgs for Vec<Value> {
    fn from_values(values: Vec<Value>) -> Result<Self, Error> {
        Ok(values)
    }
}

pub(crate) trait DynamicObject: fmt::Display + fmt::Debug + Sync + Send {
    fn get_attr(&self, name: &str) -> Option<Value>;
    fn fields(&self) -> &'static [&'static str];
    fn call_method(&self, name: &str, _args: Vec<Value>) -> Result<Value, Error> {
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            format!("object has no method named {}", name),
        ))
    }
}

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

#[derive(Clone)]
enum Repr {
    Undefined,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    Char(char),
    None,
    Shared(RcType<Shared>),
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
            Repr::Shared(val) => fmt::Debug::fmt(val, f),
        }
    }
}

#[derive(Clone)]
enum Shared {
    U128(u128),
    I128(i128),
    String(String),
    SafeString(String),
    Bytes(Vec<u8>),
    Seq(Vec<Value>),
    Map(BTreeMap<Key<'static>, Value>),
    Struct(BTreeMap<&'static str, Value>),
    // this annoyingly has basically two refcounts.  One we inherit from
    // shared, the second we have to use because the outside user of this
    // dynamic type also wants to hold on to it without having to inspect
    // into a value object.  It would be nice to be able to store this
    // adjacent to `Shared` but unfortunately a `dyn Trait` needs two
    // pointers and that incrases the size of the value type for all
    // uses.
    Dynamic(RcType<dyn DynamicObject>),
}

impl fmt::Debug for Shared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shared::U128(val) => fmt::Debug::fmt(val, f),
            Shared::I128(val) => fmt::Debug::fmt(val, f),
            Shared::String(val) => fmt::Debug::fmt(val, f),
            Shared::SafeString(val) => fmt::Debug::fmt(val, f),
            Shared::Bytes(val) => fmt::Debug::fmt(val, f),
            Shared::Seq(val) => fmt::Debug::fmt(val, f),
            Shared::Map(val) => fmt::Debug::fmt(val, f),
            Shared::Struct(val) => {
                let mut s = f.debug_struct("Struct");
                for (k, v) in val.iter() {
                    s.field(k, v);
                }
                s.finish()
            }
            Shared::Dynamic(val) => fmt::Debug::fmt(val, f),
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

impl From<Shared> for Value {
    #[inline(always)]
    fn from(val: Shared) -> Value {
        Value(Repr::Shared(RcType::new(val)))
    }
}

impl<'a> From<&'a [u8]> for Value {
    #[inline(always)]
    fn from(val: &'a [u8]) -> Self {
        Shared::Bytes(val.into()).into()
    }
}

impl<'a> From<&'a str> for Value {
    #[inline(always)]
    fn from(val: &'a str) -> Self {
        Shared::String(val.into()).into()
    }
}

impl From<String> for Value {
    #[inline(always)]
    fn from(val: String) -> Self {
        Shared::String(val).into()
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
        Shared::I128(val).into()
    }
}

impl From<u128> for Value {
    #[inline(always)]
    fn from(val: u128) -> Self {
        Shared::U128(val).into()
    }
}

impl<'a> From<Key<'a>> for Value {
    fn from(val: Key) -> Self {
        match val {
            Key::Bool(val) => val.into(),
            Key::I64(val) => val.into(),
            Key::Char(val) => val.into(),
            Key::String(val) => val.into(),
            Key::Str(val) => val.into(),
        }
    }
}

impl<K: Into<Key<'static>>, V: Into<Value>> From<BTreeMap<K, V>> for Value {
    fn from(val: BTreeMap<K, V>) -> Self {
        Shared::Map(val.into_iter().map(|(k, v)| (k.into(), v.into())).collect()).into()
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(val: Vec<T>) -> Self {
        Shared::Seq(val.into_iter().map(|x| x.into()).collect()).into()
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

impl fmt::Display for Shared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shared::I128(val) => write!(f, "{}", val),
            Shared::String(val) => write!(f, "{}", val),
            Shared::SafeString(val) => write!(f, "{}", val),
            Shared::Bytes(val) => write!(f, "{}", String::from_utf8_lossy(val)),
            Shared::Seq(values) => format_seqish(f, values.iter()),
            Shared::Map(val) => format_seqish(f, val.iter().map(|x| x.0)),
            Shared::Struct(val) => {
                for (idx, (key, _)) in val.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", key)?;
                }
                Ok(())
            }
            Shared::U128(val) => write!(f, "{}", val),
            Shared::Dynamic(x) => write!(f, "{}", x),
        }
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
            Repr::Shared(cplx) => write!(f, "{}", cplx),
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
                    concat!("tried to use ", stringify!($float), " operator on unsupported types")
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
pub(crate) fn string_concat(left: Value, right: &Value) -> Value {
    match left.0 {
        // if we're a string and we have a single reference to it, we can
        // directly append into ourselves and reconstruct the value
        Repr::Shared(mut cplx) if matches!(*cplx, Shared::String(_)) => {
            let shared = RcType::make_mut(&mut cplx);
            if let Shared::String(s) = shared {
                write!(s, "{}", right).ok();
                Value(Repr::Shared(cplx))
            } else {
                unreachable!();
            }
        }
        // otherwise we use format! to concat the two values
        _ => Value::from(format!("{}{}", left, right)),
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

primitive_try_from!(bool, {
    Primitive::Bool(val) => val,
});

primitive_try_from!(f64, {
    Primitive::F64(val) => val,
});

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

impl Value {
    /// The undefined value
    pub const UNDEFINED: Value = Value(Repr::Undefined);

    /// Creates a value from something that can be serialized.
    pub fn from_serializable<T: Serialize>(value: &T) -> Value {
        let serializer = ValueSerializer::<serde::de::value::Error>::new();
        INTERNAL_SERIALIZATION.with(|flag| {
            let old = flag.load(atomic::Ordering::Relaxed);
            flag.store(true, atomic::Ordering::Relaxed);
            let rv = Serialize::serialize(value, serializer);
            flag.store(old, atomic::Ordering::Relaxed);
            rv.unwrap()
        })
    }

    /// Creates a value from a safe string.
    pub fn from_safe_string(value: String) -> Value {
        Repr::Shared(RcType::new(Shared::SafeString(value))).into()
    }

    pub(crate) fn from_dynamic(value: RcType<dyn DynamicObject>) -> Value {
        Repr::Shared(RcType::new(Shared::Dynamic(value))).into()
    }

    /// Returns the value kind.
    pub fn kind(&self) -> ValueKind {
        match self.0 {
            Repr::Undefined => ValueKind::Undefined,
            Repr::Bool(_) => ValueKind::Bool,
            Repr::U64(_) | Repr::I64(_) | Repr::F64(_) => ValueKind::Number,
            Repr::Char(_) => ValueKind::Char,
            Repr::None => ValueKind::None,
            Repr::Shared(ref cplx) => match **cplx {
                Shared::I128(_) => ValueKind::Number,
                Shared::String(_) | Shared::SafeString(_) => ValueKind::String,
                Shared::Bytes(_) => ValueKind::Bytes,
                Shared::U128(_) => ValueKind::Number,
                Shared::Seq(_) => ValueKind::Seq,
                Shared::Map(_) => ValueKind::Map,
                Shared::Struct(_) | Shared::Dynamic(_) => ValueKind::Struct,
            },
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
            Repr::Shared(ref cplx) => match **cplx {
                Shared::I128(val) => Some(Primitive::I128(val)),
                Shared::String(ref val) => Some(Primitive::Str(val.as_str())),
                Shared::SafeString(ref val) => Some(Primitive::Str(val.as_str())),
                Shared::Bytes(ref val) => Some(Primitive::Bytes(&val[..])),
                Shared::U128(val) => Some(Primitive::U128(val)),
                _ => None,
            },
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
        matches!(&self.0, Repr::Shared(cplx) if matches!(**cplx, Shared::SafeString(_)))
    }

    /// Returns `true` if this value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(&self.0, Repr::Undefined)
    }

    /// Returns the length of the contained value.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> Option<usize> {
        if let Repr::Shared(ref cplx) = self.0 {
            match **cplx {
                Shared::String(ref s) | Shared::SafeString(ref s) => Some(s.chars().count()),
                Shared::Map(ref items) => Some(items.len()),
                Shared::Struct(ref items) => Some(items.len()),
                Shared::Seq(ref items) => Some(items.len()),
                Shared::Dynamic(ref dy) => Some(dy.fields().len()),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Looks up an attribute by attribute name.
    pub fn get_attr(&self, key: &str) -> Result<Value, Error> {
        let value = match self.0 {
            Repr::Shared(ref cplx) => match **cplx {
                Shared::Map(ref items) => {
                    let lookup_key = Key::Str(key);
                    items.get(&lookup_key).cloned()
                }
                Shared::Struct(ref items) => items.get(key).cloned(),
                Shared::Dynamic(ref dy) => dy.get_attr(key),
                _ => None,
            },
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

        if let Repr::Shared(ref cplx) = self.0 {
            match **cplx {
                Shared::Map(ref items) => return items.get(&key).cloned(),
                Shared::Struct(ref items) => {
                    if let Key::String(ref key) = key {
                        return items.get(key.as_str()).cloned();
                    }
                }
                Shared::Seq(ref items) => {
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
                Shared::Dynamic(ref dy) => {
                    if let Key::String(ref key) = key {
                        return dy.get_attr(key);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Calls a method on the value.
    pub(crate) fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, Error> {
        if let Repr::Shared(ref cplx) = self.0 {
            if let Shared::Dynamic(ref dy) = **cplx {
                return dy.call_method(name, args);
            }
        }
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            format!("object has no method named {}", name),
        ))
    }

    pub(crate) fn into_string(self) -> String {
        if let Repr::Shared(arc) = self.0 {
            match RcType::try_unwrap(arc) {
                Ok(Shared::String(s)) | Ok(Shared::SafeString(s)) => s,
                Ok(other) => other.to_string(),
                Err(arc) => arc.to_string(),
            }
        } else {
            self.to_string()
        }
    }

    pub(crate) fn try_into_vec(self) -> Result<Vec<Value>, Error> {
        if let Repr::Shared(arc) = self.0 {
            match RcType::try_unwrap(arc) {
                Ok(Shared::Seq(v)) => return Ok(v),
                Ok(_) => {}
                Err(arc) => {
                    if let Shared::Seq(v) = &*arc {
                        return Ok(v.to_vec());
                    }
                }
            }
        }
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            "cannot convert value into list",
        ))
    }

    /// Iterates over the value.
    pub(crate) fn iter(&self) -> ValueIterator {
        let value = self.clone();
        let clone = value.clone();
        let (iter_impl, len) = match &clone.0 {
            Repr::Shared(cplx) => match **cplx {
                Shared::Seq(ref items) => (ValueIteratorImpl::Seq(items.iter()), items.len()),
                Shared::Map(ref items) => (ValueIteratorImpl::Map(items.iter()), items.len()),
                Shared::Struct(ref fields) => {
                    (ValueIteratorImpl::Struct(fields.iter()), fields.len())
                }
                _ => (ValueIteratorImpl::Empty, 0),
            },
            _ => (ValueIteratorImpl::Empty, 0),
        };
        // this is insane but i'm very lazy right now to come up
        // with a better solution to hold on to the value
        ValueIterator {
            value,
            iter: unsafe { std::mem::transmute(iter_impl) },
            len,
        }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Repr::Bool(b) => serializer.serialize_bool(b),
            Repr::U64(u) => serializer.serialize_u64(u),
            Repr::I64(i) => serializer.serialize_i64(i),
            Repr::F64(f) => serializer.serialize_f64(f),
            Repr::Char(c) => serializer.serialize_char(c),
            Repr::None => serializer.serialize_unit(),
            Repr::Undefined => {
                use serde::ser::SerializeStruct;
                if in_internal_serialization() {
                    serializer.serialize_struct(UNDEFINED_MARKER, 1)?.end()
                } else {
                    serializer.serialize_unit()
                }
            }
            Repr::Shared(ref cplx) => match **cplx {
                Shared::U128(u) => serializer.serialize_u128(u),
                Shared::I128(i) => serializer.serialize_i128(i),
                Shared::String(ref s) => serializer.serialize_str(s),
                Shared::SafeString(ref val) => {
                    use serde::ser::SerializeStruct;
                    if in_internal_serialization() {
                        let mut s = serializer.serialize_struct(SAFESTRING_MARKER, 1)?;
                        s.serialize_field("value", val)?;
                        s.end()
                    } else {
                        serializer.serialize_str(val)
                    }
                }
                Shared::Bytes(ref b) => serializer.serialize_bytes(b),
                Shared::Seq(ref elements) => elements.serialize(serializer),
                Shared::Map(ref entries) => {
                    use serde::ser::SerializeMap;
                    let mut map = serializer.serialize_map(Some(entries.len()))?;
                    for (ref k, ref v) in entries.iter() {
                        map.serialize_entry(k, v)?;
                    }
                    map.end()
                }
                Shared::Struct(ref fields) => {
                    use serde::ser::SerializeStruct;
                    let mut s = serializer.serialize_struct("Struct", fields.len())?;
                    for (k, ref v) in fields.iter() {
                        s.serialize_field(k, v)?;
                    }
                    s.end()
                }
                Shared::Dynamic(ref n) => {
                    use serde::ser::SerializeMap;
                    let fields = n.fields();
                    let mut s = serializer.serialize_map(Some(fields.len()))?;
                    for k in fields {
                        let v = n.get_attr(k).unwrap_or(Value::UNDEFINED);
                        s.serialize_entry(k, &v)?;
                    }
                    s.end()
                }
            },
        }
    }
}
struct ValueSerializer<E> {
    error: PhantomData<E>,
}

impl<E> ValueSerializer<E> {
    pub fn new() -> Self {
        ValueSerializer { error: PhantomData }
    }
}

impl<E> Serializer for ValueSerializer<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    type SerializeSeq = SerializeSeq<E>;
    type SerializeTuple = SerializeTuple<E>;
    type SerializeTupleStruct = SerializeTupleStruct<E>;
    type SerializeTupleVariant = SerializeTupleVariant<E>;
    type SerializeMap = SerializeMap<E>;
    type SerializeStruct = SerializeStruct<E>;
    type SerializeStructVariant = SerializeStructVariant<E>;

    fn serialize_bool(self, v: bool) -> Result<Value, E> {
        Ok(Repr::Bool(v).into())
    }

    fn serialize_i8(self, v: i8) -> Result<Value, E> {
        Ok(Repr::I64(v as i64).into())
    }

    fn serialize_i16(self, v: i16) -> Result<Value, E> {
        Ok(Repr::I64(v as i64).into())
    }

    fn serialize_i32(self, v: i32) -> Result<Value, E> {
        Ok(Repr::I64(v as i64).into())
    }

    fn serialize_i64(self, v: i64) -> Result<Value, E> {
        Ok(Repr::I64(v).into())
    }

    fn serialize_i128(self, v: i128) -> Result<Value, E> {
        Ok(Shared::I128(v).into())
    }

    fn serialize_u8(self, v: u8) -> Result<Value, E> {
        Ok(Repr::U64(v as u64).into())
    }

    fn serialize_u16(self, v: u16) -> Result<Value, E> {
        Ok(Repr::U64(v as u64).into())
    }

    fn serialize_u32(self, v: u32) -> Result<Value, E> {
        Ok(Repr::U64(v as u64).into())
    }

    fn serialize_u64(self, v: u64) -> Result<Value, E> {
        Ok(Repr::U64(v).into())
    }

    fn serialize_u128(self, v: u128) -> Result<Value, E> {
        Ok(Shared::U128(v).into())
    }

    fn serialize_f32(self, v: f32) -> Result<Value, E> {
        Ok(Repr::F64(v as f64).into())
    }

    fn serialize_f64(self, v: f64) -> Result<Value, E> {
        Ok(Repr::F64(v).into())
    }

    fn serialize_char(self, v: char) -> Result<Value, E> {
        Ok(Repr::Char(v).into())
    }

    fn serialize_str(self, value: &str) -> Result<Value, E> {
        Ok(Shared::String(value.to_owned()).into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, E> {
        Ok(Shared::Bytes(value.to_owned()).into())
    }

    fn serialize_none(self) -> Result<Value, E> {
        Ok(Repr::None.into())
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Value, E>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, E> {
        Ok(Repr::None.into())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, E> {
        Ok(Repr::None.into())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, E> {
        Ok(Shared::String(variant.to_string()).into())
    }

    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T) -> Result<Value, E>
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
    ) -> Result<Value, E>
    where
        T: Serialize,
    {
        let mut map = BTreeMap::new();
        map.insert(Key::from(variant), value.serialize(self)?);
        Ok(Shared::Map(map).into())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, E> {
        Ok(SerializeSeq {
            elements: Vec::with_capacity(len.unwrap_or(0)),
            error: PhantomData,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, E> {
        Ok(SerializeTuple {
            elements: Vec::with_capacity(len),
            error: PhantomData,
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, E> {
        Ok(SerializeTupleStruct {
            fields: Vec::with_capacity(len),
            error: PhantomData,
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, E> {
        Ok(SerializeTupleVariant {
            name: variant,
            fields: Vec::with_capacity(len),
            error: PhantomData,
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, E> {
        Ok(SerializeMap {
            entries: BTreeMap::new(),
            key: None,
            error: PhantomData,
        })
    }

    fn serialize_struct(self, name: &'static str, _len: usize) -> Result<Self::SerializeStruct, E> {
        Ok(SerializeStruct {
            name,
            fields: BTreeMap::new(),
            error: PhantomData,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, E> {
        Ok(SerializeStructVariant {
            variant,
            map: BTreeMap::new(),
            error: PhantomData,
        })
    }
}

struct SerializeSeq<E> {
    elements: Vec<Value>,
    error: PhantomData<E>,
}

impl<E> ser::SerializeSeq for SerializeSeq<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.elements.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, E> {
        Ok(Shared::Seq(self.elements).into())
    }
}

struct SerializeTuple<E> {
    elements: Vec<Value>,
    error: PhantomData<E>,
}

impl<E> ser::SerializeTuple for SerializeTuple<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.elements.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, E> {
        Ok(Shared::Seq(self.elements).into())
    }
}

struct SerializeTupleStruct<E> {
    fields: Vec<Value>,
    error: PhantomData<E>,
}

impl<E> ser::SerializeTupleStruct for SerializeTupleStruct<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.fields.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, E> {
        Ok(Value(Repr::Shared(RcType::new(Shared::Seq(self.fields)))))
    }
}

struct SerializeTupleVariant<E> {
    name: &'static str,
    fields: Vec<Value>,
    error: PhantomData<E>,
}

impl<E> ser::SerializeTupleVariant for SerializeTupleVariant<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.fields.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, E> {
        let mut map = BTreeMap::new();
        map.insert(self.name, self.fields);
        Ok(map.into())
    }
}

struct SerializeMap<E> {
    entries: BTreeMap<Key<'static>, Value>,
    key: Option<Key<'static>>,
    error: PhantomData<E>,
}

impl<E> ser::SerializeMap for SerializeMap<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let key = key.serialize(KeySerializer::<E>::new())?;
        self.key = Some(key);
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let key = self
            .key
            .take()
            .expect("serialize_value called before serialize_key");
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.entries.insert(key, value);
        Ok(())
    }

    fn end(self) -> Result<Value, E> {
        Ok(Value(Repr::Shared(RcType::new(Shared::Map(self.entries)))))
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(&mut self, key: &K, value: &V) -> Result<(), E>
    where
        K: Serialize,
        V: Serialize,
    {
        let key = key.serialize(KeySerializer::<E>::new())?;
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.entries.insert(key, value);
        Ok(())
    }
}

struct SerializeStruct<E> {
    name: &'static str,
    fields: BTreeMap<&'static str, Value>,
    error: PhantomData<E>,
}

impl<E> ser::SerializeStruct for SerializeStruct<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.fields.insert(key, value);
        Ok(())
    }

    fn end(self) -> Result<Value, E> {
        match self.name {
            SAFESTRING_MARKER if self.fields.len() == 1 => Ok(Value::from_safe_string(
                self.fields.into_iter().next().unwrap().1.into_string(),
            )),
            UNDEFINED_MARKER => Ok(Repr::Undefined.into()),
            _ => Ok(Shared::Struct(self.fields).into()),
        }
    }
}

struct SerializeStructVariant<E> {
    variant: &'static str,
    map: BTreeMap<Key<'static>, Value>,
    error: PhantomData<E>,
}

impl<E> ser::SerializeStructVariant for SerializeStructVariant<E>
where
    E: ser::Error,
{
    type Ok = Value;
    type Error = E;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), E>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer::<E>::new())?;
        self.map.insert(Key::from(key), value);
        Ok(())
    }

    fn end(self) -> Result<Value, E> {
        let mut rv = BTreeMap::new();
        rv.insert(self.variant, Value::from(Shared::Map(self.map)));
        Ok(rv.into())
    }
}

pub(crate) struct ValueIterator {
    // this is a hack that keeps a reference.  ValueIteratorImpl is highly
    // unsafe.  This needs to be fixed.
    #[allow(unused)]
    value: Value,
    iter: ValueIteratorImpl<'static>,
    len: usize,
}

impl Iterator for ValueIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|x| {
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

enum ValueIteratorImpl<'a> {
    Empty,
    Seq(std::slice::Iter<'a, Value>),
    Map(std::collections::btree_map::Iter<'a, Key<'a>, Value>),
    Struct(std::collections::btree_map::Iter<'a, &'static str, Value>),
}

impl<'a> ValueIteratorImpl<'a> {
    fn next(&mut self) -> Option<Value> {
        match self {
            ValueIteratorImpl::Empty => None,
            ValueIteratorImpl::Seq(iter) => iter.next().cloned(),
            ValueIteratorImpl::Map(iter) => iter.next().map(|x| x.0.clone().into()),
            ValueIteratorImpl::Struct(iter) => iter.next().map(|x| Value::from(*x.0)),
        }
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
        "impossible operation: tried to use + operator on unsupported types"
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
fn test_string_key_lookup() {
    let mut m = BTreeMap::new();
    m.insert(Key::String("foo".into()), Value::from(42));
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
