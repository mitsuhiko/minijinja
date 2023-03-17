use std::convert::TryFrom;

use serde::ser::{Impossible, Serialize, Serializer};

use crate::key::{Key, StaticKey};
use crate::utils::SerializationFailed;

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
    type Error = SerializationFailed;

    type SerializeSeq = Impossible<StaticKey, SerializationFailed>;
    type SerializeTuple = Impossible<StaticKey, SerializationFailed>;
    type SerializeTupleStruct = Impossible<StaticKey, SerializationFailed>;
    type SerializeTupleVariant = Impossible<StaticKey, SerializationFailed>;
    type SerializeMap = Impossible<StaticKey, SerializationFailed>;
    type SerializeStruct = Impossible<StaticKey, SerializationFailed>;
    type SerializeStructVariant = Impossible<StaticKey, SerializationFailed>;

    fn serialize_bool(self, v: bool) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i16(self, v: i16) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i32(self, v: i32) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_i64(self, v: i64) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::I64(v))
    }

    #[cold]
    fn serialize_i128(self, _: i128) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type i128")
    }

    fn serialize_u8(self, v: u8) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u16(self, v: u16) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u32(self, v: u32) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::I64(v as i64))
    }

    fn serialize_u64(self, v: u64) -> Result<StaticKey, SerializationFailed> {
        match Key::try_from(v) {
            Ok(rv) => Ok(rv),
            Err(_) => panic!("out of bounds for i64"),
        }
    }

    #[cold]
    fn serialize_u128(self, _: u128) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type u128")
    }

    #[cold]
    fn serialize_f32(self, _: f32) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type f32")
    }

    #[cold]
    fn serialize_f64(self, _: f64) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type f64")
    }

    fn serialize_char(self, v: char) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::Char(v))
    }

    fn serialize_str(self, value: &str) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::make_string_key(value))
    }

    #[cold]
    fn serialize_bytes(self, _value: &[u8]) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type bytes")
    }

    #[cold]
    fn serialize_none(self) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type unit")
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<StaticKey, SerializationFailed>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    #[cold]
    fn serialize_unit(self) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type unit")
    }

    #[cold]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<StaticKey, SerializationFailed> {
        panic!("unsupported key type unit")
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<StaticKey, SerializationFailed> {
        Ok(Key::Str(variant))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<StaticKey, SerializationFailed>
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
    ) -> Result<StaticKey, SerializationFailed>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    #[cold]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, SerializationFailed> {
        panic!("sequences as keys are not supported")
    }

    #[cold]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, SerializationFailed> {
        panic!("tuples as keys are not supported")
    }

    #[cold]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, SerializationFailed> {
        panic!("tuple structs as keys are not supported")
    }

    #[cold]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, SerializationFailed> {
        panic!("tuple variants as keys are not supported",)
    }

    #[cold]
    fn serialize_map(
        self,
        _jlen: Option<usize>,
    ) -> Result<Self::SerializeMap, SerializationFailed> {
        panic!("maps as keys are not supported")
    }

    #[cold]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, SerializationFailed> {
        panic!("structs as keys are not supported")
    }

    #[cold]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, SerializationFailed> {
        panic!("structs as keys are not supported")
    }
}
