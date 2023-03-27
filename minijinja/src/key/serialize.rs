use std::convert::TryFrom;
use std::fmt;

use serde::ser::{Impossible, Serialize, Serializer};

use crate::key::{Key, StaticKey};

/// Value is not a valid key.
#[derive(Debug)]
pub struct InvalidKey;

impl std::error::Error for InvalidKey {}

impl fmt::Display for InvalidKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid key")
    }
}

impl serde::ser::Error for InvalidKey {
    #[track_caller]
    #[cold]
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        let _ = msg;
        InvalidKey
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
    type Error = InvalidKey;

    type SerializeSeq = Impossible<StaticKey, InvalidKey>;
    type SerializeTuple = Impossible<StaticKey, InvalidKey>;
    type SerializeTupleStruct = Impossible<StaticKey, InvalidKey>;
    type SerializeTupleVariant = Impossible<StaticKey, InvalidKey>;
    type SerializeMap = Impossible<StaticKey, InvalidKey>;
    type SerializeStruct = Impossible<StaticKey, InvalidKey>;
    type SerializeStructVariant = Impossible<StaticKey, InvalidKey>;

    fn serialize_bool(self, v: bool) -> Result<StaticKey, InvalidKey> {
        Ok(Key::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<StaticKey, InvalidKey> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i16(self, v: i16) -> Result<StaticKey, InvalidKey> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i32(self, v: i32) -> Result<StaticKey, InvalidKey> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i64(self, v: i64) -> Result<StaticKey, InvalidKey> {
        Ok(Key::I64(v))
    }

    #[cold]
    fn serialize_i128(self, _: i128) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    fn serialize_u8(self, v: u8) -> Result<StaticKey, InvalidKey> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u16(self, v: u16) -> Result<StaticKey, InvalidKey> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u32(self, v: u32) -> Result<StaticKey, InvalidKey> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u64(self, v: u64) -> Result<StaticKey, InvalidKey> {
        match Key::try_from(v) {
            Ok(rv) => Ok(rv),
            Err(_) => Err(InvalidKey),
        }
    }

    #[cold]
    fn serialize_u128(self, _: u128) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_f32(self, _: f32) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_f64(self, _: f64) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    fn serialize_char(self, v: char) -> Result<StaticKey, InvalidKey> {
        Ok(Key::Char(v))
    }

    fn serialize_str(self, value: &str) -> Result<StaticKey, InvalidKey> {
        Ok(Key::make_string_key(value))
    }

    #[cold]
    fn serialize_bytes(self, _value: &[u8]) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_none(self) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<StaticKey, InvalidKey>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    #[cold]
    fn serialize_unit(self) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<StaticKey, InvalidKey> {
        Err(InvalidKey)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<StaticKey, InvalidKey> {
        Ok(Key::Str(variant))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<StaticKey, InvalidKey>
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
    ) -> Result<StaticKey, InvalidKey>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    #[cold]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_map(self, _jlen: Option<usize>) -> Result<Self::SerializeMap, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, InvalidKey> {
        Err(InvalidKey)
    }

    #[cold]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, InvalidKey> {
        Err(InvalidKey)
    }
}
