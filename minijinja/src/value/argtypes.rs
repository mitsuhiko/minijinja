use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};

use crate::error::{Error, ErrorKind};
use crate::key::{Key, StaticKey};
use crate::value::{
    Arc, MapType, Object, Packed, SeqObject, StringType, Value, ValueKind, ValueRepr,
};
use crate::vm::State;

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
/// arity of 5 parameters.
///
/// For each argument the conversion is performed via the [`ArgType`]
/// trait which is implemented for many common types.  For manual
/// conversions the [`from_args`] utility should be used.
pub trait FunctionArgs<'a> {
    /// The output type of the function arguments.
    type Output;

    /// Converts to function arguments from a slice of values.
    #[doc(hidden)]
    fn from_values(state: Option<&'a State>, values: &'a [Value]) -> Result<Self::Output, Error>;
}

/// Utility function to convert a slice of values into arguments.
///
/// This performs the same conversion that [`Function`](crate::functions::Function)
/// performs.  It exists so that you one can leverage the same functionality when
/// implementing [`Object::call_method`](crate::value::Object::call_method).
///
/// ```
/// use minijinja::value::from_args;
/// # use minijinja::value::Value;
/// # fn foo() -> Result<(), minijinja::Error> {
/// # let args = vec![Value::from("foo"), Value::from(42i64)]; let args = &args[..];
///
/// // args is &[Value]
/// let (string, num): (&str, i64) = from_args(args)?;
/// # Ok(()) } fn main() { foo().unwrap(); }
/// ```
///
/// Note that only value conversions are supported which means that `&State` is not
/// a valid conversion type.
#[inline(always)]
pub fn from_args<'a, Args>(values: &'a [Value]) -> Result<Args, Error>
where
    Args: FunctionArgs<'a, Output = Args>,
{
    Args::from_values(None, values)
}

/// A trait implemented by all filter/test argument types.
///
/// This trait is used by [`FunctionArgs`].  It's implemented for many common
/// types that are typically passed to filters, tests or functions.  It's
/// implemented for the following types:
///
/// * eval state: [`&State`](crate::State) (see below for notes)
/// * unsigned integers: [`u8`], [`u16`], [`u32`], [`u64`], [`u128`], [`usize`]
/// * signed integers: [`i8`], [`i16`], [`i32`], [`i64`], [`i128`]
/// * floats: [`f32`], [`f64`]
/// * bool: [`bool`]
/// * string: [`String`], [`&str`], `Cow<'_, str>`, [`char`]
/// * bytes: [`&[u8]`][`slice`]
/// * values: [`Value`], `&Value`
/// * vectors: [`Vec<T>`]
/// * sequences: [`&dyn SeqObject`](crate::value::SeqObject)
///
/// The type is also implemented for optional values (`Option<T>`) which is used
/// to encode optional parameters to filters, functions or tests.  Additionally
/// it's implemented for [`Rest<T>`] which is used to encode the remaining arguments
/// of a function call.
///
/// ## Notes on Borrowing
///
/// Note on that there is an important difference between `String` and `&str`:
/// the former will be valid for all values and an implicit conversion to string
/// via [`ToString`] will take place, for the latter only values which are already
/// strings will be passed.  A compromise between the two is `Cow<'_, str>` which
/// will behave like `String` but borrows when possible.
///
/// Byte slices will borrow out of values carrying bytes or strings.  In the latter
/// case the utf-8 bytes are returned.
///
/// There are also further restrictions imposed on borrowing in some situations.
/// For instance you cannot implicitly borrow out of sequences which means that
/// for instance `Vec<&str>` is not a legal argument.
///
/// ## Notes on State
///
/// When `&State` is used, it does not consume a passed parameter.  This means that
/// a filter that takes `(&State, String)` actually only has one argument.  The
/// state is passed implicitly.
pub trait ArgType<'a> {
    /// The output type of this argument.
    type Output;

    #[doc(hidden)]
    fn from_value(value: Option<&'a Value>) -> Result<Self::Output, Error>;

    #[doc(hidden)]
    fn from_value_owned(_value: Value) -> Result<Self::Output, Error> {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "type conversion is not legal in this situation (implicit borrow)",
        ))
    }

    #[doc(hidden)]
    fn from_state_and_value(
        _state: Option<&'a State>,
        value: Option<&'a Value>,
    ) -> Result<(Self::Output, usize), Error> {
        Ok((ok!(Self::from_value(value)), 1))
    }

    #[doc(hidden)]
    #[inline(always)]
    fn from_state_and_values(
        state: Option<&'a State>,
        values: &'a [Value],
        offset: usize,
    ) -> Result<(Self::Output, usize), Error> {
        Self::from_state_and_value(state, values.get(offset))
    }
}

macro_rules! tuple_impls {
    ( $( $name:ident )* * $rest_name:ident ) => {
        impl<'a, $($name,)* $rest_name> FunctionArgs<'a> for ($($name,)* $rest_name,)
            where $($name: ArgType<'a>,)* $rest_name: ArgType<'a>
        {
            type Output = ($($name::Output,)* $rest_name::Output ,);

            fn from_values(state: Option<&'a State>, values: &'a [Value]) -> Result<Self::Output, Error> {
                #![allow(non_snake_case, unused)]
                let mut idx = 0;
                $(
                    let ($name, offset) = ok!($name::from_state_and_value(state, values.get(idx)));
                    idx += offset;
                )*
                let ($rest_name, offset) = ok!($rest_name::from_state_and_values(state, values, idx));
                idx += offset;
                if values.get(idx).is_some() {
                    Err(Error::from(ErrorKind::TooManyArguments))
                } else {
                    Ok(( $($name,)* $rest_name,))
                }
            }
        }
    };
}

impl<'a> FunctionArgs<'a> for () {
    type Output = ();

    fn from_values(_state: Option<&'a State>, values: &'a [Value]) -> Result<Self::Output, Error> {
        if values.is_empty() {
            Ok(())
        } else {
            Err(Error::from(ErrorKind::TooManyArguments))
        }
    }
}

tuple_impls! { *A }
tuple_impls! { A *B }
tuple_impls! { A B *C }
tuple_impls! { A B C *D }
tuple_impls! { A B C D *E }

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
        ValueRepr::String(Arc::new(val.into()), StringType::Normal).into()
    }
}

impl From<String> for Value {
    #[inline(always)]
    fn from(val: String) -> Self {
        ValueRepr::String(Arc::new(val), StringType::Normal).into()
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

impl<'a> From<Key<'a>> for Value {
    fn from(val: Key) -> Self {
        match val {
            Key::Bool(val) => val.into(),
            Key::I64(val) => val.into(),
            Key::Char(val) => val.into(),
            Key::String(val) => ValueRepr::String(val, StringType::Normal).into(),
            Key::Str(val) => val.into(),
        }
    }
}

impl<V: Into<Value>> FromIterator<V> for Value {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let vec = iter.into_iter().map(|v| v.into()).collect();

        ValueRepr::Seq(Arc::new(vec)).into()
    }
}

impl<K: Into<StaticKey>, V: Into<Value>> FromIterator<(K, V)> for Value {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let map = iter
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();

        ValueRepr::Map(Arc::new(map), MapType::Normal).into()
    }
}

impl<K: Into<StaticKey>, V: Into<Value>> From<BTreeMap<K, V>> for Value {
    fn from(val: BTreeMap<K, V>) -> Self {
        val.into_iter().map(|(k, v)| (k.into(), v.into())).collect()
    }
}

impl<K: Into<StaticKey>, V: Into<Value>> From<HashMap<K, V>> for Value {
    fn from(val: HashMap<K, V>) -> Self {
        val.into_iter().map(|(k, v)| (k.into(), v.into())).collect()
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(val: Vec<T>) -> Self {
        val.into_iter().map(|v| v.into()).collect()
    }
}

impl<T: Object> From<Arc<T>> for Value {
    fn from(object: Arc<T>) -> Self {
        Value::from(object as Arc<dyn Object>)
    }
}

impl From<Arc<String>> for Value {
    fn from(value: Arc<String>) -> Self {
        Value(ValueRepr::String(value, StringType::Normal))
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

impl From<i128> for Value {
    #[inline(always)]
    fn from(val: i128) -> Self {
        ValueRepr::I128(Packed(val)).into()
    }
}

impl From<u128> for Value {
    #[inline(always)]
    fn from(val: u128) -> Self {
        ValueRepr::U128(Packed(val)).into()
    }
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
value_from!(Arc<Vec<u8>>, Bytes);
value_from!(Arc<Vec<Value>>, Seq);
value_from!(Arc<dyn Object>, Dynamic);

fn unsupported_conversion(kind: ValueKind, target: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("cannot convert {kind} to {target}"),
    )
}

macro_rules! primitive_try_from {
    ($ty:ident, {
        $($pat:pat $(if $if_expr:expr)? => $expr:expr,)*
    }) => {
        impl TryFrom<Value> for $ty {
            type Error = Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                match value.0 {
                    $($pat $(if $if_expr)? => TryFrom::try_from($expr).ok(),)*
                    _ => None
                }.ok_or_else(|| unsupported_conversion(value.kind(), stringify!($ty)))
            }
        }

        impl<'a> ArgType<'a> for $ty {
            type Output = Self;
            fn from_value(value: Option<&Value>) -> Result<Self, Error> {
                match value {
                    Some(value) => TryFrom::try_from(value.clone()),
                    None => Err(Error::from(ErrorKind::MissingArgument))
                }
            }

            fn from_value_owned(value: Value) -> Result<Self, Error> {
                TryFrom::try_from(value)
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
            ValueRepr::I128(val) => val.0,
            ValueRepr::U128(val) => val.0,
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
primitive_try_from!(char, {
    ValueRepr::Char(val) => val,
});
primitive_try_from!(f32, {
    ValueRepr::F64(val) => val as f32,
});
primitive_try_from!(f64, {
    ValueRepr::F64(val) => val,
});

impl<'a> ArgType<'a> for &str {
    type Output = &'a str;

    fn from_value(value: Option<&'a Value>) -> Result<Self::Output, Error> {
        match value {
            Some(value) => value
                .as_str()
                .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "value is not a string")),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }
}

impl<'a> ArgType<'a> for &[u8] {
    type Output = &'a [u8];

    fn from_value(value: Option<&'a Value>) -> Result<Self::Output, Error> {
        match value {
            Some(value) => value
                .as_bytes()
                .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "value is not in bytes")),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }
}

impl<'a> ArgType<'a> for &dyn SeqObject {
    type Output = &'a dyn SeqObject;

    fn from_value(value: Option<&'a Value>) -> Result<Self::Output, Error> {
        match value {
            Some(value) => value
                .as_seq()
                .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "value is not a sequence")),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }
}

impl<'a, T: ArgType<'a>> ArgType<'a> for Option<T> {
    type Output = Option<T::Output>;

    fn from_value(value: Option<&'a Value>) -> Result<Self::Output, Error> {
        match value {
            Some(value) => {
                if value.is_undefined() || value.is_none() {
                    Ok(None)
                } else {
                    T::from_value(Some(value)).map(Some)
                }
            }
            None => Ok(None),
        }
    }

    fn from_value_owned(value: Value) -> Result<Self::Output, Error> {
        if value.is_undefined() || value.is_none() {
            Ok(None)
        } else {
            T::from_value_owned(value).map(Some)
        }
    }
}

impl<'a> ArgType<'a> for Cow<'_, str> {
    type Output = Cow<'a, str>;

    #[inline(always)]
    fn from_value(value: Option<&'a Value>) -> Result<Cow<'a, str>, Error> {
        match value {
            Some(value) => Ok(match value.0 {
                ValueRepr::String(ref s, _) => Cow::Borrowed(s.as_str()),
                _ => Cow::Owned(value.to_string()),
            }),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }
}

impl<'a> ArgType<'a> for &Value {
    type Output = &'a Value;

    #[inline(always)]
    fn from_value(value: Option<&'a Value>) -> Result<&'a Value, Error> {
        match value {
            Some(value) => Ok(value),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }
}

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

impl<'a, T: ArgType<'a, Output = T>> ArgType<'a> for Rest<T> {
    type Output = Self;

    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        Ok(Rest(ok!(value
            .iter()
            .map(|v| T::from_value(Some(v)))
            .collect::<Result<_, _>>())))
    }

    fn from_state_and_values(
        _state: Option<&'a State>,
        values: &'a [Value],
        offset: usize,
    ) -> Result<(Self, usize), Error> {
        let args = values.get(offset..).unwrap_or_default();
        Ok((
            Rest(ok!(args
                .iter()
                .map(|v| T::from_value(Some(v)))
                .collect::<Result<_, _>>())),
            args.len(),
        ))
    }
}

impl<'a> ArgType<'a> for Value {
    type Output = Self;

    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            Some(value) => Ok(value.clone()),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }

    fn from_value_owned(value: Value) -> Result<Self, Error> {
        Ok(value)
    }
}

impl<'a> ArgType<'a> for String {
    type Output = Self;

    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            Some(value) => Ok(value.to_string()),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }

    fn from_value_owned(value: Value) -> Result<Self, Error> {
        Ok(value.to_string())
    }
}

impl<'a, T: ArgType<'a, Output = T>> ArgType<'a> for Vec<T> {
    type Output = Vec<T>;

    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            None => Ok(Vec::new()),
            Some(value) => {
                let seq = ok!(value
                    .as_seq()
                    .ok_or_else(|| { Error::new(ErrorKind::InvalidOperation, "not a sequence") }));
                let mut rv = Vec::new();
                for value in seq.iter() {
                    rv.push(ok!(T::from_value_owned(value)));
                }
                Ok(rv)
            }
        }
    }

    fn from_value_owned(value: Value) -> Result<Self, Error> {
        let seq = ok!(value
            .as_seq()
            .ok_or_else(|| { Error::new(ErrorKind::InvalidOperation, "not a sequence") }));
        let mut rv = Vec::new();
        for value in seq.iter() {
            rv.push(ok!(T::from_value_owned(value)));
        }
        Ok(rv)
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
