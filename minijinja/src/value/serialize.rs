use std::fmt;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::cell::{Cell, RefCell};

use serde::{ser, Serialize, Serializer};

use crate::utils::{untrusted_size_hint, OnDrop};
use crate::value::{
    value_map_with_capacity, value_optimization,
    Packed, StringType, ValueBox, OwnedValueBoxMap, ValueRepr,
};

use super::ArcCow;

// We use in-band signalling to roundtrip some internal values.  This is
// not ideal but unfortunately there is no better system in serde today.
const VALUE_HANDLE_MARKER: &str = "\x01__minijinja_ValueBoxHandle";

thread_local! {
    static INTERNAL_SERIALIZATION: Cell<bool> = Cell::new(false);

    // This should be an AtomicU64 but sadly 32bit targets do not necessarily have
    // AtomicU64 available.
    static LAST_VALUE_HANDLE: Cell<u32> = Cell::new(0);
    static VALUE_HANDLES: RefCell<BTreeMap<u32, ValueBox>> = RefCell::new(BTreeMap::new());
}

fn mark_internal_serialization() -> impl Drop {
    let old = INTERNAL_SERIALIZATION.with(|flag| {
        let old = flag.get();
        flag.set(true);
        old
    });
    OnDrop::new(move || {
        if !old {
            INTERNAL_SERIALIZATION.with(|flag| flag.set(false));
        }
    })
}

/// Transforms a serializable value to a value object.
///
/// This neither fails nor panics.  For objects that cannot be represented
/// the value might be represented as a half broken error object.
pub fn transform<T: Serialize>(value: T) -> ValueBox {
    match value.serialize(ValueBoxSerializer) {
        Ok(rv) => rv,
        Err(invalid) => ValueRepr::Invalid(invalid.0.into()).into(),
    }
}

/// Function that returns true when serialization for [`ValueBox`] is taking place.
///
/// MiniJinja internally creates [`ValueBox`] objects from all values passed to the
/// engine.  It does this by going through the regular serde serialization trait.
/// In some cases users might want to customize the serialization specifically for
/// MiniJinja because they want to tune the object for the template engine
/// independently of what is normally serialized to disk.
///
/// This function returns `true` when MiniJinja is serializing to [`ValueBox`] and
/// `false` otherwise.  You can call this within your own [`Serialize`]
/// implementation to change the output format.
///
/// This is particularly useful as serialization for MiniJinja does not need to
/// support deserialization.  So it becomes possible to completely change what
/// gets sent there, even at the cost of serializing something that cannot be
/// deserialized.
pub fn serializing_for_value() -> bool {
    INTERNAL_SERIALIZATION.with(|flag| flag.get())
}

impl ValueBox {
    /// Creates a value from something that can be serialized.
    ///
    /// This is the method that MiniJinja will generally use whenever a serializable
    /// object is passed to one of the APIs that internally want to create a value.
    /// For instance this is what [`context!`](crate::context) and
    /// [`render`](crate::Template::render) will use.
    ///
    /// During serialization of the value, [`serializing_for_value`] will return
    /// `true` which makes it possible to customize serialization for MiniJinja.
    /// For more information see [`serializing_for_value`].
    ///
    /// ```
    /// # use minijinja::value::ValueBox;
    /// let val = ValueBox::from_serializable(&vec![1, 2, 3]);
    /// ```
    ///
    /// This method does not fail but it might return a value that is not valid.  Such
    /// values will when operated on fail in the template engine in most situations.
    /// This for instance can happen if the underlying implementation of [`Serialize`]
    /// fails.  There are also cases where invalid objects are silently hidden in the
    /// engine today.  This is for instance the case for when keys are used in hash maps
    /// that the engine cannot deal with.  Invalid values are considered an implementation
    /// detail.  There is currently no API to validate a value.
    pub fn from_serializable<T: Serialize>(value: &T) -> ValueBox {
        let _serialization_guard = mark_internal_serialization();
        let _optimization_guard = value_optimization();
        transform(value)
    }
}

impl Serialize for ValueBox {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // enable round tripping of values
        if serializing_for_value() {
            let handle = LAST_VALUE_HANDLE.with(|x| {
                // we are okay with overflowing the handle here because these values only
                // live for a very short period of time and it's not likely that you run out
                // of an entire u32 worth of handles in a single serialization operation.
                // This lets us stick the handle into a unit variant in the serde data model.
                let rv = x.get().wrapping_add(1);
                x.set(rv);
                rv
            });
            VALUE_HANDLES.with(|handles| handles.borrow_mut().insert(handle, self.clone()));
            return serializer.serialize_unit_variant(
                VALUE_HANDLE_MARKER,
                handle,
                VALUE_HANDLE_MARKER,
            );
        }

        match self.0 {
            ValueRepr::Bool(b) => serializer.serialize_bool(b),
            ValueRepr::U64(u) => serializer.serialize_u64(u),
            ValueRepr::I64(i) => serializer.serialize_i64(i),
            ValueRepr::F64(f) => serializer.serialize_f64(f),
            ValueRepr::None | ValueRepr::Undefined | ValueRepr::Invalid(_) => {
                serializer.serialize_unit()
            }
            ValueRepr::U128(u) => serializer.serialize_u128(u.0),
            ValueRepr::I128(i) => serializer.serialize_i128(i.0),
            ValueRepr::String(ref s, _) => serializer.serialize_str(s),
            ValueRepr::Bytes(ref b) => serializer.serialize_bytes(b),
            ValueRepr::Seq(ref s) => {
                use serde::ser::SerializeSeq;
                let mut seq = ok!(serializer.serialize_seq(Some(s.item_count())));
                for item in s.iter() {
                    ok!(seq.serialize_element(&item));
                }
                seq.end()
            },
            ValueRepr::Map(ref s, _) => {
                use serde::ser::SerializeMap;
                let mut map = ok!(serializer.serialize_map(None));
                if let Some(fields) = s.static_fields() {
                    for k in fields {
                        let v = s.get_field(&ValueBox::from(*k)).unwrap_or(ValueBox::UNDEFINED);
                        ok!(map.serialize_entry(k, &v));
                    }
                } else {
                    for k in s.fields() {
                        let v = s.get_field(&k).unwrap_or(ValueBox::UNDEFINED);
                        ok!(map.serialize_entry(&k, &v));
                    }
                }
                map.end()
            }
            ValueRepr::Dynamic(ref dy) => dy.value().serialize(serializer),
        }
    }
}

#[derive(Debug)]
pub struct InvalidValueBox(Arc<str>);

impl std::error::Error for InvalidValueBox {}

impl fmt::Display for InvalidValueBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl serde::ser::Error for InvalidValueBox {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        InvalidValueBox(Arc::from(msg.to_string()))
    }
}

pub struct ValueBoxSerializer;

impl Serializer for ValueBoxSerializer {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    type SerializeSeq = SerializeSeq;
    type SerializeTuple = SerializeTuple;
    type SerializeTupleStruct = SerializeTupleStruct;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeStruct;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::Bool(v).into())
    }

    fn serialize_i8(self, v: i8) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i16(self, v: i16) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i32(self, v: i32) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::I64(v as i64).into())
    }

    fn serialize_i64(self, v: i64) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::I64(v).into())
    }

    fn serialize_i128(self, v: i128) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::I128(Packed(v)).into())
    }

    fn serialize_u8(self, v: u8) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u16(self, v: u16) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u32(self, v: u32) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::U64(v as u64).into())
    }

    fn serialize_u64(self, v: u64) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::U64(v).into())
    }

    fn serialize_u128(self, v: u128) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::U128(Packed(v)).into())
    }

    fn serialize_f32(self, v: f32) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::F64(v as f64).into())
    }

    fn serialize_f64(self, v: f64) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::F64(v).into())
    }

    fn serialize_char(self, v: char) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::String(ArcCow::from(v.to_string()), StringType::Normal).into())
    }

    fn serialize_str(self, value: &str) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::String(ArcCow::from(value.to_owned()), StringType::Normal).into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::Bytes(ArcCow::from(value)).into())
    }

    fn serialize_none(self) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<ValueBox, InvalidValueBox>
    where
        T: Serialize,
    {
        Ok(transform(value))
    }

    fn serialize_unit(self) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueRepr::None.into())
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<ValueBox, InvalidValueBox> {
        if name == VALUE_HANDLE_MARKER && variant == VALUE_HANDLE_MARKER {
            Ok(VALUE_HANDLES.with(|handles| {
                let mut handles = handles.borrow_mut();
                handles
                    .remove(&variant_index)
                    .expect("value handle not in registry")
            }))
        } else {
            Ok(ValueBox::from(variant))
        }
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<ValueBox, InvalidValueBox>
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
    ) -> Result<ValueBox, InvalidValueBox>
    where
        T: Serialize,
    {
        let mut map = value_map_with_capacity(1);
        map.insert(ValueBox::from(variant), transform(value));
        Ok(ValueBox::from_map_object(map))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, InvalidValueBox> {
        Ok(SerializeSeq {
            elements: Vec::with_capacity(untrusted_size_hint(len.unwrap_or(0))),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, InvalidValueBox> {
        Ok(SerializeTuple {
            elements: Vec::with_capacity(untrusted_size_hint(len)),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, InvalidValueBox> {
        Ok(SerializeTupleStruct {
            fields: Vec::with_capacity(untrusted_size_hint(len)),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, InvalidValueBox> {
        Ok(SerializeTupleVariant {
            name: variant,
            fields: Vec::with_capacity(untrusted_size_hint(len)),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, InvalidValueBox> {
        Ok(SerializeMap {
            entries: value_map_with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, InvalidValueBox> {
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
    ) -> Result<Self::SerializeStructVariant, InvalidValueBox> {
        Ok(SerializeStructVariant {
            variant,
            map: value_map_with_capacity(len),
        })
    }
}

pub struct SerializeSeq {
    elements: Vec<ValueBox>,
}

impl ser::SerializeSeq for SerializeSeq {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        self.elements.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueBox::from_seq_object(self.elements))
    }
}

pub struct SerializeTuple {
    elements: Vec<ValueBox>,
}

impl ser::SerializeTuple for SerializeTuple {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        self.elements.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueBox::from_seq_object(self.elements))
    }
}

pub struct SerializeTupleStruct {
    fields: Vec<ValueBox>,
}

impl ser::SerializeTupleStruct for SerializeTupleStruct {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        self.fields.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueBox::from_seq_object(self.fields))
    }
}

pub struct SerializeTupleVariant {
    name: &'static str,
    fields: Vec<ValueBox>,
}

impl ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        self.fields.push(transform(value));
        Ok(())
    }

    fn end(self) -> Result<ValueBox, InvalidValueBox> {
        let mut map = value_map_with_capacity(1);
        map.insert(
            ValueBox::from(self.name),
            ValueBox::from_seq_object(self.fields)
        );
        Ok(ValueBox::from_map_object(map))
    }
}

pub struct SerializeMap {
    entries: OwnedValueBoxMap,
    key: Option<ValueBox>,
}

impl ser::SerializeMap for SerializeMap {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        match key.serialize(ValueBoxSerializer) {
            Ok(key) => self.key = Some(key),
            Err(_) => self.key = None,
        }
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        if let Some(key) = self.key.take() {
            self.entries.insert(key, transform(value));
        }
        Ok(())
    }

    fn end(self) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueBox::from_map_object(self.entries))
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), InvalidValueBox>
    where
        K: Serialize,
        V: Serialize,
    {
        if let Ok(key) = key.serialize(ValueBoxSerializer) {
            self.entries.insert(key, transform(value));
        }
        Ok(())
    }
}

pub struct SerializeStruct {
    fields: OwnedValueBoxMap,
}

impl ser::SerializeStruct for SerializeStruct {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        self.fields.insert(ValueBox::from(key), transform(value));
        Ok(())
    }

    fn end(self) -> Result<ValueBox, InvalidValueBox> {
        Ok(ValueBox::from_map_object(self.fields))
    }
}

pub struct SerializeStructVariant {
    variant: &'static str,
    map: OwnedValueBoxMap,
}

impl ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = ValueBox;
    type Error = InvalidValueBox;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), InvalidValueBox>
    where
        T: Serialize,
    {
        self.map.insert(ValueBox::from(key), transform(value));
        Ok(())
    }

    fn end(self) -> Result<ValueBox, InvalidValueBox> {
        let mut rv = BTreeMap::new();
        rv.insert(self.variant, ValueBox::from_map_object(self.map));
        Ok(rv.into())
    }
}
