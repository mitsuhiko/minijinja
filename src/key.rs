use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::num::TryFromIntError;

use serde::ser::{self, Impossible, Serialize, Serializer};

use crate::error::{Error, ErrorKind};
use crate::value::{Primitive, Value};

/// Represents a key in a value's map.
#[derive(Debug, Clone)]
pub enum Key<'a> {
    Bool(bool),
    I64(i64),
    Char(char),
    String(String),
    Str(&'a str),
}

#[derive(PartialOrd, Ord, Eq, PartialEq)]
pub enum InternalKeyRef<'a> {
    Bool(bool),
    I64(i64),
    Char(char),
    Str(&'a str),
}

impl<'a> Key<'a> {
    pub fn as_key_ref(&self) -> InternalKeyRef<'_> {
        match *self {
            Key::Bool(x) => InternalKeyRef::Bool(x),
            Key::I64(x) => InternalKeyRef::I64(x),
            Key::Char(x) => InternalKeyRef::Char(x),
            Key::String(ref x) => InternalKeyRef::Str(x.as_str()),
            Key::Str(x) => InternalKeyRef::Str(x),
        }
    }

    pub fn from_borrowed_value(value: &'a Value) -> Result<Key<'a>, Error> {
        match value
            .as_primitive()
            .ok_or_else(|| Error::from(ErrorKind::NonKey))?
        {
            Primitive::Bool(v) => Ok(Key::Bool(v)),
            Primitive::U64(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Primitive::U128(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Primitive::I64(v) => Ok(Key::I64(v)),
            Primitive::I128(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Primitive::F64(_) => Err(ErrorKind::NonKey.into()),
            Primitive::Char(c) => Ok(Key::Char(c)),
            Primitive::Str(s) => Ok(Key::Str(s)),
            Primitive::Bytes(_) | Primitive::None | Primitive::Undefined => {
                Err(ErrorKind::NonKey.into())
            }
        }
    }
}

impl TryFrom<Value> for Key<'static> {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value
            .as_primitive()
            .ok_or_else(|| Error::from(ErrorKind::NonKey))?
        {
            Primitive::Bool(v) => Ok(Key::Bool(v)),
            Primitive::U64(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Primitive::U128(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Primitive::I64(v) => Ok(Key::I64(v)),
            Primitive::I128(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            Primitive::F64(_) => Err(ErrorKind::NonKey.into()),
            Primitive::Char(c) => Ok(Key::Char(c)),
            Primitive::Str(s) => Ok(Key::String(s.to_string())),
            Primitive::Bytes(_) | Primitive::None | Primitive::Undefined => {
                Err(ErrorKind::NonKey.into())
            }
        }
    }
}

impl<'a> PartialEq for Key<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.as_key_ref().eq(&other.as_key_ref())
    }
}

impl<'a> Eq for Key<'a> {}

impl<'a> PartialOrd for Key<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_key_ref().partial_cmp(&other.as_key_ref())
    }
}

impl<'a> Ord for Key<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_key_ref().cmp(&other.as_key_ref())
    }
}

type StaticKey = Key<'static>;

impl<'a> fmt::Display for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Bool(val) => write!(f, "{}", val),
            Key::I64(val) => write!(f, "{}", val),
            Key::Char(val) => write!(f, "{}", val),
            Key::String(val) => write!(f, "{}", val),
            Key::Str(val) => write!(f, "{}", val),
        }
    }
}

macro_rules! key_from {
    ($src:ty, $dst:ident) => {
        impl From<$src> for Key<'static> {
            #[inline(always)]
            fn from(val: $src) -> Self {
                Key::$dst(val as _)
            }
        }
    };
}

key_from!(bool, Bool);
key_from!(u8, I64);
key_from!(u16, I64);
key_from!(u32, I64);
key_from!(i8, I64);
key_from!(i16, I64);
key_from!(i32, I64);
key_from!(i64, I64);
key_from!(char, Char);

impl TryFrom<u64> for Key<'static> {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        TryFrom::try_from(value).map(Key::I64)
    }
}

impl<'a> From<&'a str> for Key<'static> {
    #[inline(always)]
    fn from(value: &'a str) -> Self {
        Key::String(value.to_string())
    }
}

impl<'a> Serialize for Key<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            Key::Bool(b) => serializer.serialize_bool(b),
            Key::I64(i) => serializer.serialize_i64(i),
            Key::Char(c) => serializer.serialize_char(c),
            Key::String(ref s) => serializer.serialize_str(s),
            Key::Str(s) => serializer.serialize_str(s),
        }
    }
}

pub struct KeySerializer;

impl Serializer for KeySerializer {
    type Ok = StaticKey;
    type Error = Error;

    type SerializeSeq = Impossible<StaticKey, Error>;
    type SerializeTuple = Impossible<StaticKey, Error>;
    type SerializeTupleStruct = Impossible<StaticKey, Error>;
    type SerializeTupleVariant = Impossible<StaticKey, Error>;
    type SerializeMap = Impossible<StaticKey, Error>;
    type SerializeStruct = Impossible<StaticKey, Error>;
    type SerializeStructVariant = Impossible<StaticKey, Error>;

    fn serialize_bool(self, v: bool) -> Result<StaticKey, Error> {
        Ok(Key::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<StaticKey, Error> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i16(self, v: i16) -> Result<StaticKey, Error> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i32(self, v: i32) -> Result<StaticKey, Error> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i64(self, v: i64) -> Result<StaticKey, Error> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i128(self, _: i128) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type i128"))
    }

    fn serialize_u8(self, v: u8) -> Result<StaticKey, Error> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u16(self, v: u16) -> Result<StaticKey, Error> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u32(self, v: u32) -> Result<StaticKey, Error> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u64(self, v: u64) -> Result<StaticKey, Error> {
        Key::try_from(v).map_err(|_| ser::Error::custom("out of bounds for i64"))
    }

    fn serialize_u128(self, _: u128) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type u128"))
    }

    fn serialize_f32(self, _: f32) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type f32"))
    }

    fn serialize_f64(self, _: f64) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type f64"))
    }

    fn serialize_char(self, v: char) -> Result<StaticKey, Error> {
        Ok(Key::Char(v))
    }

    fn serialize_str(self, value: &str) -> Result<StaticKey, Error> {
        Ok(Key::String(value.to_string()))
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type bytes"))
    }

    fn serialize_none(self) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type unit"))
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<StaticKey, Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type unit"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<StaticKey, Error> {
        Err(ser::Error::custom("unsupported key type unit"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<StaticKey, Error> {
        Ok(Key::String(variant.into()))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<StaticKey, Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<StaticKey, Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(ser::Error::custom("sequences as keys are not supported"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Err(ser::Error::custom("tuples as keys are not supported"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(ser::Error::custom(
            "tuple structs as keys are not supported",
        ))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(ser::Error::custom(
            "tuple variants as keys are not supported",
        ))
    }

    fn serialize_map(self, _jlen: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(ser::Error::custom("maps as keys are not supported"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Err(ser::Error::custom("structs as keys are not supported"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(ser::Error::custom("structs as keys are not supported"))
    }
}
