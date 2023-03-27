use std::collections::BTreeMap;
use std::fmt;

use serde::{ser, Serialize, Serializer};

use crate::key::{Key, KeySerializer, StaticKey};
use crate::value::{
    value_map_with_capacity, Arc, MapType, Packed, StringType, Value, ValueMap, ValueRepr,
    VALUE_HANDLES, VALUE_HANDLE_MARKER,
};

#[derive(Debug)]
pub struct InvalidValue(Arc<String>);

impl std::error::Error for InvalidValue {}

impl fmt::Display for InvalidValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::ser::Error for InvalidValue {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        InvalidValue(Arc::new(msg.to_string()))
    }
}

/// Transforms a serializable value to a value object.
///
/// This neither fails nor panics.  For objects that cannot be represented
/// the value might be represented as a half broken error object.
pub fn transform<T: Serialize>(value: T) -> Value {
    match value.serialize(ValueSerializer) {
        Ok(rv) => rv,
        Err(invalid) => ValueRepr::Invalid(invalid.0).into(),
    }
}

pub struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = InvalidValue;

    type SerializeSeq = SerializeSeq;
    type SerializeTuple = SerializeTuple;
    type SerializeTupleStruct = SerializeTupleStruct;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeStruct;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::Bool(v).into())
    }

    fn serialize_i8(self, v: i8) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i16(self, v: i16) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i32(self, v: i32) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i64(self, v: i64) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::I64(v).into())
    }

    fn serialize_i128(self, v: i128) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::I128(Packed(v)).into())
    }

    fn serialize_u8(self, v: u8) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u16(self, v: u16) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u32(self, v: u32) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u64(self, v: u64) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::U64(v).into())
    }

    fn serialize_u128(self, v: u128) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::U128(Packed(v)).into())
    }

    fn serialize_f32(self, v: f32) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::F64(v as f64).into())
    }

    fn serialize_f64(self, v: f64) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::F64(v).into())
    }

    fn serialize_char(self, v: char) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::Char(v).into())
    }

    fn serialize_str(self, value: &str) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::String(Arc::new(value.to_owned()), StringType::Normal).into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::Bytes(Arc::new(value.to_owned())).into())
    }

    fn serialize_none(self) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Value, InvalidValue>
    where
        T: Serialize,
    {
        Ok(transform(value))
    }

    fn serialize_unit(self) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, InvalidValue> {
        if name == VALUE_HANDLE_MARKER && variant == VALUE_HANDLE_MARKER {
            Ok(VALUE_HANDLES.with(|handles| {
                let mut handles = handles.borrow_mut();
                handles
                    .remove(&variant_index)
                    .expect("value handle not in registry")
            }))
        } else {
            Ok(Value::from(variant))
        }
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, InvalidValue>
    where
        T: Serialize,
    {
        Ok(transform(value))
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, InvalidValue>
    where
        T: Serialize,
    {
        let mut map = value_map_with_capacity(1);
        map.insert(Key::Str(variant), transform(value));
        Ok(ValueRepr::Map(Arc::new(map), MapType::Normal).into())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, InvalidValue> {
        Ok(SerializeSeq {
            elements: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, InvalidValue> {
        Ok(SerializeTuple {
            elements: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, InvalidValue> {
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
    ) -> Result<Self::SerializeTupleVariant, InvalidValue> {
        Ok(SerializeTupleVariant {
            name: variant,
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, InvalidValue> {
        Ok(SerializeMap {
            entries: value_map_with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, InvalidValue> {
        Ok(SerializeStruct {
            fields: value_map_with_capacity(len),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, InvalidValue> {
        Ok(SerializeStructVariant {
            variant,
            map: value_map_with_capacity(len),
        })
    }
}

pub struct SerializeSeq {
    elements: Vec<Value>,
}

impl ser::SerializeSeq for SerializeSeq {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        self.elements.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::Seq(Arc::new(self.elements)).into())
    }
}

pub struct SerializeTuple {
    elements: Vec<Value>,
}

impl ser::SerializeTuple for SerializeTuple {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        self.elements.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::Seq(Arc::new(self.elements)).into())
    }
}

pub struct SerializeTupleStruct {
    fields: Vec<Value>,
}

impl ser::SerializeTupleStruct for SerializeTupleStruct {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        self.fields.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        Ok(Value(ValueRepr::Seq(Arc::new(self.fields))))
    }
}

pub struct SerializeTupleVariant {
    name: &'static str,
    fields: Vec<Value>,
}

impl ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        self.fields.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        let mut map = value_map_with_capacity(1);
        map.insert(
            Key::Str(self.name),
            Value(ValueRepr::Seq(self.fields.into())),
        );
        Ok(Value(ValueRepr::Map(map.into(), MapType::Normal)))
    }
}

pub struct SerializeMap {
    entries: ValueMap,
    key: Option<StaticKey>,
}

impl ser::SerializeMap for SerializeMap {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        match key.serialize(KeySerializer) {
            Ok(key) => self.key = Some(key),
            Err(_) => self.key = None,
        }
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        if let Some(key) = self.key.take() {
            self.entries.insert(key, transform(value));
        }
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        Ok(Value(ValueRepr::Map(
            Arc::new(self.entries),
            MapType::Normal,
        )))
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), InvalidValue>
    where
        K: Serialize,
        V: Serialize,
    {
        if let Ok(key) = key.serialize(KeySerializer) {
            self.entries.insert(key, transform(value));
        }
        Ok(())
    }
}

pub struct SerializeStruct {
    fields: ValueMap,
}

impl ser::SerializeStruct for SerializeStruct {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        self.fields.insert(Key::Str(key), transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        Ok(ValueRepr::Map(Arc::new(self.fields), MapType::Normal).into())
    }
}

pub struct SerializeStructVariant {
    variant: &'static str,
    map: ValueMap,
}

impl ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        self.map.insert(Key::Str(key), transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        let mut rv = BTreeMap::new();
        rv.insert(
            self.variant,
            Value::from(ValueRepr::Map(Arc::new(self.map), MapType::Normal)),
        );
        Ok(rv.into())
    }
}
