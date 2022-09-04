use std::collections::BTreeMap;

use serde::{ser, Serialize, Serializer};

use crate::error::Error;
use crate::key::{Key, KeySerializer, StaticKey};
use crate::value::{Arc, Value, ValueMap, ValueRepr, VALUE_HANDLES, VALUE_HANDLE_MARKER};

pub struct ValueSerializer;

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
        Ok(ValueRepr::Bool(v).into())
    }

    fn serialize_i8(self, v: i8) -> Result<Value, Error> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i16(self, v: i16) -> Result<Value, Error> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i32(self, v: i32) -> Result<Value, Error> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i64(self, v: i64) -> Result<Value, Error> {
        Ok(ValueRepr::I64(v).into())
    }

    fn serialize_i128(self, v: i128) -> Result<Value, Error> {
        Ok(ValueRepr::I128(Arc::new(v)).into())
    }

    fn serialize_u8(self, v: u8) -> Result<Value, Error> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u16(self, v: u16) -> Result<Value, Error> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u32(self, v: u32) -> Result<Value, Error> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u64(self, v: u64) -> Result<Value, Error> {
        Ok(ValueRepr::U64(v).into())
    }

    fn serialize_u128(self, v: u128) -> Result<Value, Error> {
        Ok(ValueRepr::U128(Arc::new(v)).into())
    }

    fn serialize_f32(self, v: f32) -> Result<Value, Error> {
        Ok(ValueRepr::F64(v as f64).into())
    }

    fn serialize_f64(self, v: f64) -> Result<Value, Error> {
        Ok(ValueRepr::F64(v).into())
    }

    fn serialize_char(self, v: char) -> Result<Value, Error> {
        Ok(ValueRepr::Char(v).into())
    }

    fn serialize_str(self, value: &str) -> Result<Value, Error> {
        Ok(ValueRepr::String(Arc::new(value.to_owned())).into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, Error> {
        Ok(ValueRepr::Bytes(Arc::new(value.to_owned())).into())
    }

    fn serialize_none(self) -> Result<Value, Error> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Value, Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(ValueRepr::String(Arc::new(variant.to_string())).into())
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
        Ok(ValueRepr::Map(Arc::new(map)).into())
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

pub struct SerializeSeq {
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
        Ok(ValueRepr::Seq(Arc::new(self.elements)).into())
    }
}

pub struct SerializeTuple {
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
        Ok(ValueRepr::Seq(Arc::new(self.elements)).into())
    }
}

pub struct SerializeTupleStruct {
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
        Ok(Value(ValueRepr::Seq(Arc::new(self.fields))))
    }
}

pub struct SerializeTupleVariant {
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

pub struct SerializeMap {
    entries: ValueMap,
    key: Option<StaticKey>,
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
        Ok(Value(ValueRepr::Map(Arc::new(self.entries))))
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

pub struct SerializeStruct {
    name: &'static str,
    fields: ValueMap,
}

impl ser::SerializeStruct for SerializeStruct {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.fields.insert(Key::Str(key), value);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        match self.name {
            VALUE_HANDLE_MARKER => {
                let handle_id = match self.fields.get(&Key::Str("handle")) {
                    Some(&Value(ValueRepr::U64(handle_id))) => handle_id as usize,
                    _ => panic!("bad handle reference in value roundtrip"),
                };
                Ok(VALUE_HANDLES.with(|handles| {
                    let mut handles = handles.borrow_mut();
                    handles
                        .remove(&handle_id)
                        .expect("value handle not in registry")
                }))
            }
            _ => Ok(ValueRepr::Map(Arc::new(self.fields)).into()),
        }
    }
}

pub struct SerializeStructVariant {
    variant: &'static str,
    map: ValueMap,
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
        rv.insert(
            self.variant,
            Value::from(ValueRepr::Map(Arc::new(self.map))),
        );
        Ok(rv.into())
    }
}
