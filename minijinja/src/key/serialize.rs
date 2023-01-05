use std::convert::TryFrom;

use serde::ser::{self, Impossible, Serialize, Serializer};

use crate::error::Error;
use crate::key::{Key, StaticKey};

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
        Ok(Key::I64(v))
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
        Ok(Key::make_string_key(value))
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
        Ok(Key::Str(variant))
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
