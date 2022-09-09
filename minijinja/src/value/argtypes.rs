use std::borrow::Cow;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};

use crate::error::{Error, ErrorKind};
use crate::key::{Key, StaticKey};
use crate::value::{Arc, Value, ValueRepr};

/// A utility trait that represents the return value of functions and filters.
///
/// It's implemented for the following types:
///
/// * `Rv` where `Rv` implements `Into<Value>`
/// * `Result<Rv, Error>` where `Rv` implements `Into<Value>`
///
/// The equivalent for test functions is [`TestResult`](crate::tests::TestResult).
pub trait FunctionResult {
    #[doc(hidden)]
    fn into_result(self) -> Result<Value, Error>;
}

impl<I: Into<Value>> FunctionResult for Result<I, Error> {
    fn into_result(self) -> Result<Value, Error> {
        self.map(Into::into)
    }
}

impl<I: Into<Value>> FunctionResult for I {
    fn into_result(self) -> Result<Value, Error> {
        Ok(self.into())
    }
}

/// Helper trait representing valid filter, test and function arguments.
///
/// Since it's more convenient to write filters and tests with concrete
/// types instead of values, this helper trait exists to automatically
/// perform this conversion.  It is implemented for functions up to an
/// arity of 4 parameters.
///
/// For each argument the conversion is performed via the [`ArgType`]
/// trait which is implemented for many common types.
pub trait FunctionArgs<'a>: Sized {
    /// Converts to function arguments from a slice of values.
    fn from_values(values: &'a [Value]) -> Result<Self, Error>;
}

/// A trait implemented by all filter/test argument types.
///
/// This trait is used by [`FunctionArgs`].  It's implemented for many common
/// types that are typically passed to filters, tests or functions.  It's
/// implemented for the following types:
///
/// * unsigned integers: [`u8`], [`u16`], [`u32`], [`u64`], [`u128`], [`usize`]
/// * signed integers: [`i8`], [`i16`], [`i32`], [`i64`], [`i128`]
/// * floats: [`f64`]
/// * bool: [`bool`]
/// * string: [`String`]
/// * values: [`Value`]
/// * vectors: [`Vec<T>`]
///
/// The type is also implemented for optional values (`Option<T>`) which is used
/// to encode optional parameters to filters, functions or tests.  Additionally
/// it's implemented for [`Rest<T>`] which is used to encode the remaining arguments
/// of a function call.
pub trait ArgType<'a>: Sized {
    #[doc(hidden)]
    fn from_value(value: Option<&'a Value>) -> Result<Self, Error>;

    #[doc(hidden)]
    #[inline(always)]
    fn from_rest_values(_values: &'a [Value]) -> Result<Option<Self>, Error> {
        Ok(None)
    }
}

macro_rules! tuple_impls {
    ( $( $name:ident )* $(; ( $($alt_name:ident)* ) $rest_name:ident)? ) => {
        impl<'a, $($name),*> FunctionArgs<'a> for ($($name,)*)
            where $($name: ArgType<'a>,)*
        {
            fn from_values(values: &'a [Value]) -> Result<Self, Error> {
                #![allow(non_snake_case, unused)]
                let arg_count = 0 $(
                    + { let $name = (); 1 }
                )*;

                $(
                    let rest_values = values.get(arg_count - 1..).unwrap_or_default();
                    if let Some(rest) = $rest_name::from_rest_values(rest_values)? {
                        let mut idx = 0;
                        $(
                            let $alt_name = ArgType::from_value(values.get(idx))?;
                            idx += 1;
                        )*
                        return Ok(( $($alt_name,)* rest ,));
                    }
                )?

                if values.len() > arg_count {
                    return Err(Error::new(
                        ErrorKind::InvalidArguments,
                        "received unexpected extra arguments",
                    ));
                }
                {
                    let mut idx = 0;
                    $(
                        let $name = ArgType::from_value(values.get(idx))?;
                        idx += 1;
                    )*
                    Ok(( $($name,)* ))
                }
            }
        }
    };
}

tuple_impls! {}
tuple_impls! { A; () A }
tuple_impls! { A B; (A) B }
tuple_impls! { A B C; (A B) C }
tuple_impls! { A B C D; (A B C) D }

impl From<ValueRepr> for Value {
    #[inline(always)]
    fn from(val: ValueRepr) -> Value {
        Value(val)
    }
}

impl<'a> From<&'a [u8]> for Value {
    #[inline(always)]
    fn from(val: &'a [u8]) -> Self {
        ValueRepr::Bytes(Arc::new(val.into())).into()
    }
}

impl<'a> From<&'a str> for Value {
    #[inline(always)]
    fn from(val: &'a str) -> Self {
        ValueRepr::String(Arc::new(val.into())).into()
    }
}

impl From<String> for Value {
    #[inline(always)]
    fn from(val: String) -> Self {
        ValueRepr::String(Arc::new(val)).into()
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
        ValueRepr::None.into()
    }
}

impl From<i128> for Value {
    #[inline(always)]
    fn from(val: i128) -> Self {
        ValueRepr::I128(Arc::new(val)).into()
    }
}

impl From<u128> for Value {
    #[inline(always)]
    fn from(val: u128) -> Self {
        ValueRepr::U128(Arc::new(val)).into()
    }
}

impl<'a> From<Key<'a>> for Value {
    fn from(val: Key) -> Self {
        match val {
            Key::Bool(val) => val.into(),
            Key::I64(val) => val.into(),
            Key::Char(val) => val.into(),
            Key::String(val) => ValueRepr::String(val).into(),
            Key::Str(val) => val.into(),
        }
    }
}

impl<K: Into<StaticKey>, V: Into<Value>> From<BTreeMap<K, V>> for Value {
    fn from(val: BTreeMap<K, V>) -> Self {
        ValueRepr::Map(Arc::new(
            val.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        ))
        .into()
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(val: Vec<T>) -> Self {
        ValueRepr::Seq(Arc::new(val.into_iter().map(|x| x.into()).collect())).into()
    }
}

macro_rules! value_from {
    ($src:ty, $dst:ident) => {
        impl From<$src> for Value {
            #[inline(always)]
            fn from(val: $src) -> Self {
                ValueRepr::$dst(val as _).into()
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

macro_rules! primitive_try_from {
    ($ty:ident, {
        $($pat:pat $(if $if_expr:expr)? => $expr:expr,)*
    }) => {

        impl TryFrom<Value> for $ty {
            type Error = Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                let opt = match value.0 {
                    $($pat $(if $if_expr)? => TryFrom::try_from($expr).ok(),)*
                    _ => None
                };
                opt.ok_or_else(|| {
                    Error::new(
                        ErrorKind::ImpossibleOperation,
                        format!("cannot convert {} to {}", value.kind(), stringify!($ty))
                    )
                })
            }
        }

        impl<'a> ArgType<'a> for $ty {
            fn from_value(value: Option<&Value>) -> Result<Self, Error> {
                match value {
                    Some(value) => TryFrom::try_from(value.clone()),
                    None => Err(Error::new(ErrorKind::UndefinedError, "missing argument"))
                }
            }
        }

        impl<'a> ArgType<'a> for Option<$ty> {
            fn from_value(value: Option<&Value>) -> Result<Self, Error> {
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
            ValueRepr::Bool(val) => val as usize,
            ValueRepr::I64(val) => val,
            ValueRepr::U64(val) => val,
            // for the intention here see Key::from_borrowed_value
            ValueRepr::F64(val) if (val as i64 as f64 == val) => val as i64,
            ValueRepr::I128(ref val) => **val,
            ValueRepr::U128(ref val) => **val,
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
    ValueRepr::Bool(val) => val,
});

primitive_try_from!(f64, {
    ValueRepr::F64(val) => val,
});

/// Utility type to capture remaining arguments.
///
/// In some cases you might want to have a variadic function.  In that case
/// you can define the last argument to a [`Filter`](crate::filters::Filter),
/// [`Test`](crate::tests::Test) or [`Function`](crate::functions::Function)
/// this way.  The `Rest<T>` type will collect all the remaining arguments
/// here.  It's implemented for all [`ArgType`]s.  The type itself deref's
/// into the inner vector.
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::State;
/// use minijinja::value::Rest;
///
/// fn sum(_state: &State, values: Rest<i64>) -> i64 {
///     values.iter().sum()
/// }
/// ```
#[derive(Debug)]
pub struct Rest<T>(pub Vec<T>);

impl<T> Deref for Rest<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Rest<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, T: ArgType<'a>> ArgType<'a> for Rest<T> {
    fn from_value(_value: Option<&'a Value>) -> Result<Self, Error> {
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            "cannot collect remaining arguments in this argument position",
        ))
    }

    #[inline(always)]
    fn from_rest_values(values: &'a [Value]) -> Result<Option<Self>, Error> {
        Ok(Some(Rest(
            values
                .iter()
                .map(|v| ArgType::from_value(Some(v)))
                .collect::<Result<_, _>>()?,
        )))
    }
}

impl<'a> ArgType<'a> for Value {
    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            Some(value) => Ok(value.clone()),
            None => Err(Error::new(ErrorKind::UndefinedError, "missing argument")),
        }
    }
}

impl<'a> ArgType<'a> for Option<Value> {
    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            Some(value) => {
                if value.is_undefined() || value.is_none() {
                    Ok(None)
                } else {
                    Ok(Some(value.clone()))
                }
            }
            None => Ok(None),
        }
    }
}

impl<'a> ArgType<'a> for String {
    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            Some(value) => Ok(value.to_string()),
            None => Err(Error::new(ErrorKind::UndefinedError, "missing argument")),
        }
    }
}

impl<'a> ArgType<'a> for Option<String> {
    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            Some(value) => {
                if value.is_undefined() || value.is_none() {
                    Ok(None)
                } else {
                    Ok(Some(value.to_string()))
                }
            }
            None => Ok(None),
        }
    }
}

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

impl<'a, T: ArgType<'a>> ArgType<'a> for Vec<T> {
    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            None => Ok(Vec::new()),
            Some(values) => {
                let values = values.as_slice()?;
                let mut rv = Vec::new();
                for value in values {
                    rv.push(ArgType::from_value(Some(value))?);
                }
                Ok(rv)
            }
        }
    }
}
