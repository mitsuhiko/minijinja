use std::collections::BTreeMap;

use serde::{ser, Serialize, Serializer};

use crate::key::{Key, KeySerializer, StaticKey};
use crate::utils::SerializationFailed;
use crate::value::{
    value_map_with_capacity, Arc, MapType, Packed, StringType, Value, ValueMap, ValueRepr,
    VALUE_HANDLES, VALUE_HANDLE_MARKER,
};
pub struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = SerializationFailed;

    type SerializeSeq = SerializeSeq;
    type SerializeTuple = SerializeTuple;
    type SerializeTupleStruct = SerializeTupleStruct;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeStruct;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::Bool(v).into())
    }

    fn serialize_i8(self, v: i8) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i16(self, v: i16) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i32(self, v: i32) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i64(self, v: i64) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::I64(v).into())
    }

    fn serialize_i128(self, v: i128) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::I128(Packed(v)).into())
    }

    fn serialize_u8(self, v: u8) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u16(self, v: u16) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u32(self, v: u32) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u64(self, v: u64) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::U64(v).into())
    }

    fn serialize_u128(self, v: u128) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::U128(Packed(v)).into())
    }

    fn serialize_f32(self, v: f32) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::F64(v as f64).into())
    }

    fn serialize_f64(self, v: f64) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::F64(v).into())
    }

    fn serialize_char(self, v: char) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::Char(v).into())
    }

    fn serialize_str(self, value: &str) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::String(Arc::new(value.to_owned()), StringType::Normal).into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::Bytes(Arc::new(value.to_owned())).into())
    }

    fn serialize_none(self) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Value, SerializationFailed>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, SerializationFailed> {
        Ok(Value::from(variant))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, SerializationFailed>
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
    ) -> Result<Value, SerializationFailed>
    where
        T: Serialize,
    {
        let mut map = value_map_with_capacity(1);
        map.insert(Key::Str(variant), ok!(value.serialize(self)));
        Ok(ValueRepr::Map(Arc::new(map), MapType::Normal).into())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, SerializationFailed> {
        Ok(SerializeSeq {
            elements: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, SerializationFailed> {
        Ok(SerializeTuple {
            elements: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, SerializationFailed> {
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
    ) -> Result<Self::SerializeTupleVariant, SerializationFailed> {
        Ok(SerializeTupleVariant {
            name: variant,
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, SerializationFailed> {
        Ok(SerializeMap {
            entries: value_map_with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, SerializationFailed> {
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
    ) -> Result<Self::SerializeStructVariant, SerializationFailed> {
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
    type Error = SerializationFailed;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let value = ok!(value.serialize(ValueSerializer));
        self.elements.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::Seq(Arc::new(self.elements)).into())
    }
}

pub struct SerializeTuple {
    elements: Vec<Value>,
}

impl ser::SerializeTuple for SerializeTuple {
    type Ok = Value;
    type Error = SerializationFailed;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let value = ok!(value.serialize(ValueSerializer));
        self.elements.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::Seq(Arc::new(self.elements)).into())
    }
}

pub struct SerializeTupleStruct {
    fields: Vec<Value>,
}

impl ser::SerializeTupleStruct for SerializeTupleStruct {
    type Ok = Value;
    type Error = SerializationFailed;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let value = ok!(value.serialize(ValueSerializer));
        self.fields.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, SerializationFailed> {
        Ok(Value(ValueRepr::Seq(Arc::new(self.fields))))
    }
}

pub struct SerializeTupleVariant {
    name: &'static str,
    fields: Vec<Value>,
}

impl ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = Value;
    type Error = SerializationFailed;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let value = ok!(value.serialize(ValueSerializer));
        self.fields.push(value);
        Ok(())
    }

    fn end(self) -> Result<Value, SerializationFailed> {
        if self.name == VALUE_HANDLE_MARKER && self.fields.len() == 1 {
            let handle_id = match self.fields.get(0) {
                Some(&Value(ValueRepr::U64(handle_id))) => handle_id as usize,
                _ => panic!("bad handle reference in value roundtrip"),
            };
            Ok(VALUE_HANDLES.with(|handles| {
                let mut handles = handles.borrow_mut();
                handles
                    .remove(&handle_id)
                    .expect("value handle not in registry")
            }))
        } else {
            let mut map = value_map_with_capacity(1);
            map.insert(
                Key::Str(self.name),
                Value(ValueRepr::Seq(self.fields.into())),
            );
            Ok(Value(ValueRepr::Map(map.into(), MapType::Normal)))
        }
    }
}

pub struct SerializeMap {
    entries: ValueMap,
    key: Option<StaticKey>,
}

impl ser::SerializeMap for SerializeMap {
    type Ok = Value;
    type Error = SerializationFailed;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let key = ok!(key.serialize(KeySerializer));
        self.key = Some(key);
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let key = self
            .key
            .take()
            .expect("serialize_value called before serialize_key");
        let value = ok!(value.serialize(ValueSerializer));
        self.entries.insert(key, value);
        Ok(())
    }

    fn end(self) -> Result<Value, SerializationFailed> {
        Ok(Value(ValueRepr::Map(
            Arc::new(self.entries),
            MapType::Normal,
        )))
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), SerializationFailed>
    where
        K: Serialize,
        V: Serialize,
    {
        let key = ok!(key.serialize(KeySerializer));
        let value = ok!(value.serialize(ValueSerializer));
        self.entries.insert(key, value);
        Ok(())
    }
}

pub struct SerializeStruct {
    fields: ValueMap,
}

impl ser::SerializeStruct for SerializeStruct {
    type Ok = Value;
    type Error = SerializationFailed;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let value = ok!(value.serialize(ValueSerializer));
        self.fields.insert(Key::Str(key), value);
        Ok(())
    }

    fn end(self) -> Result<Value, SerializationFailed> {
        Ok(ValueRepr::Map(Arc::new(self.fields), MapType::Normal).into())
    }
}

pub struct SerializeStructVariant {
    variant: &'static str,
    map: ValueMap,
}

impl ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = Value;
    type Error = SerializationFailed;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerializationFailed>
    where
        T: Serialize,
    {
        let value = ok!(value.serialize(ValueSerializer));
        self.map.insert(Key::Str(key), value);
        Ok(())
    }

    fn end(self) -> Result<Value, SerializationFailed> {
        let mut rv = BTreeMap::new();
        rv.insert(
            self.variant,
            Value::from(ValueRepr::Map(Arc::new(self.map), MapType::Normal)),
        );
        Ok(rv.into())
    }
}
