use std::fmt;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::cell::{Cell, RefCell};

use serde::{ser, Serialize, Serializer};

use crate::utils::{untrusted_size_hint, OnDrop};
use crate::value::{
    value_map_with_capacity, value_optimization,
    KeyRef, MapType, Packed, StringType, Value, OwnedValueMap, ValueBuf, ObjectKind,
};

// We use in-band signalling to roundtrip some internal values.  This is
// not ideal but unfortunately there is no better system in serde today.
const VALUE_HANDLE_MARKER: &str = "\x01__minijinja_ValueHandle";

thread_local! {
    static INTERNAL_SERIALIZATION: Cell<bool> = Cell::new(false);

    // This should be an AtomicU64 but sadly 32bit targets do not necessarily have
    // AtomicU64 available.
    static LAST_VALUE_HANDLE: Cell<u32> = Cell::new(0);
    static VALUE_HANDLES: RefCell<BTreeMap<u32, Value>> = RefCell::new(BTreeMap::new());
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
pub fn transform<T: Serialize>(value: T) -> Value {
    match value.serialize(ValueSerializer) {
        Ok(rv) => rv,
        Err(invalid) => ValueBuf::Invalid(invalid.0).into(),
    }
}

/// Function that returns true when serialization for [`Value`] is taking place.
///
/// MiniJinja internally creates [`Value`] objects from all values passed to the
/// engine.  It does this by going through the regular serde serialization trait.
/// In some cases users might want to customize the serialization specifically for
/// MiniJinja because they want to tune the object for the template engine
/// independently of what is normally serialized to disk.
///
/// This function returns `true` when MiniJinja is serializing to [`Value`] and
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

impl Value {
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
    /// # use minijinja::value::Value;
    /// let val = Value::from_serializable(&vec![1, 2, 3]);
    /// ```
    ///
    /// This method does not fail but it might return a value that is not valid.  Such
    /// values will when operated on fail in the template engine in most situations.
    /// This for instance can happen if the underlying implementation of [`Serialize`]
    /// fails.  There are also cases where invalid objects are silently hidden in the
    /// engine today.  This is for instance the case for when keys are used in hash maps
    /// that the engine cannot deal with.  Invalid values are considered an implementation
    /// detail.  There is currently no API to validate a value.
    pub fn from_serializable<T: Serialize>(value: &T) -> Value {
        let _serialization_guard = mark_internal_serialization();
        let _optimization_guard = value_optimization();
        transform(value)
    }
}

impl Serialize for Value {
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
            ValueBuf::Bool(b) => serializer.serialize_bool(b),
            ValueBuf::U64(u) => serializer.serialize_u64(u),
            ValueBuf::I64(i) => serializer.serialize_i64(i),
            ValueBuf::F64(f) => serializer.serialize_f64(f),
            ValueBuf::None | ValueBuf::Undefined | ValueBuf::Invalid(_) => {
                serializer.serialize_unit()
            }
            ValueBuf::U128(u) => serializer.serialize_u128(u.0),
            ValueBuf::I128(i) => serializer.serialize_i128(i.0),
            ValueBuf::String(ref s, _) => serializer.serialize_str(s),
            ValueBuf::Bytes(ref b) => serializer.serialize_bytes(b),
            ValueBuf::Seq(ref elements) => elements.serialize(serializer),
            ValueBuf::Map(ref entries, _) => {
                use serde::ser::SerializeMap;
                let mut map = ok!(serializer.serialize_map(Some(entries.len())));
                for (ref k, ref v) in entries.iter() {
                    ok!(map.serialize_entry(k, v));
                }
                map.end()
            }
            ValueBuf::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Plain => serializer.serialize_str(&dy.to_string()),
                ObjectKind::Value(v) => v.serialize(serializer),
                ObjectKind::Seq(s) => {
                    use serde::ser::SerializeSeq;
                    let mut seq = ok!(serializer.serialize_seq(Some(s.item_count())));
                    for item in s.iter() {
                        ok!(seq.serialize_element(&item));
                    }
                    seq.end()
                }
                ObjectKind::Struct(s) => {
                    use serde::ser::SerializeMap;
                    let mut map = ok!(serializer.serialize_map(None));
                    if let Some(fields) = s.static_fields() {
                        for k in fields {
                            let v = s.get_field(k).unwrap_or(Value::UNDEFINED);
                            ok!(map.serialize_entry(k, &v));
                        }
                    } else {
                        for k in s.fields() {
                            let v = s.get_field(&k).unwrap_or(Value::UNDEFINED);
                            ok!(map.serialize_entry(&*k as &str, &v));
                        }
                    }
                    map.end()
                }
            },
        }
    }
}

#[derive(Debug)]
pub struct InvalidValue(Arc<str>);

impl std::error::Error for InvalidValue {}

impl fmt::Display for InvalidValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl serde::ser::Error for InvalidValue {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        InvalidValue(Arc::from(msg.to_string()))
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
        Ok(ValueBuf::Bool(v).into())
    }

    fn serialize_i8(self, v: i8) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::I64(v as i64).into())
    }

    fn serialize_i16(self, v: i16) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::I64(v as i64).into())
    }

    fn serialize_i32(self, v: i32) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::I64(v as i64).into())
    }

    fn serialize_i64(self, v: i64) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::I64(v).into())
    }

    fn serialize_i128(self, v: i128) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::I128(Packed(v)).into())
    }

    fn serialize_u8(self, v: u8) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::U64(v as u64).into())
    }

    fn serialize_u16(self, v: u16) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::U64(v as u64).into())
    }

    fn serialize_u32(self, v: u32) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::U64(v as u64).into())
    }

    fn serialize_u64(self, v: u64) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::U64(v).into())
    }

    fn serialize_u128(self, v: u128) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::U128(Packed(v)).into())
    }

    fn serialize_f32(self, v: f32) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::F64(v as f64).into())
    }

    fn serialize_f64(self, v: f64) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::F64(v).into())
    }

    fn serialize_char(self, v: char) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::String(Arc::from(v.to_string()), StringType::Normal).into())
    }

    fn serialize_str(self, value: &str) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::String(Arc::from(value.to_owned()), StringType::Normal).into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::Bytes(Arc::from(value)).into())
    }

    fn serialize_none(self) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::None.into())
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Value, InvalidValue>
    where
        T: Serialize,
    {
        Ok(transform(value))
    }

    fn serialize_unit(self) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::None.into())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::None.into())
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
        map.insert(KeyRef::Str(variant), transform(value));
        Ok(ValueBuf::Map(Arc::new(map), MapType::Normal).into())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, InvalidValue> {
        Ok(SerializeSeq {
            elements: Vec::with_capacity(untrusted_size_hint(len.unwrap_or(0))),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, InvalidValue> {
        Ok(SerializeTuple {
            elements: Vec::with_capacity(untrusted_size_hint(len)),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, InvalidValue> {
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
    ) -> Result<Self::SerializeTupleVariant, InvalidValue> {
        Ok(SerializeTupleVariant {
            name: variant,
            fields: Vec::with_capacity(untrusted_size_hint(len)),
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
        Ok(ValueBuf::Seq(Arc::from(self.elements)).into())
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
        Ok(ValueBuf::Seq(Arc::from(self.elements)).into())
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
        Ok(Value(ValueBuf::Seq(Arc::from(self.fields))))
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
            KeyRef::Str(self.name),
            Value(ValueBuf::Seq(self.fields.into())),
        );
        Ok(Value(ValueBuf::Map(map.into(), MapType::Normal)))
    }
}

pub struct SerializeMap {
    entries: OwnedValueMap,
    key: Option<Value>,
}

impl ser::SerializeMap for SerializeMap {
    type Ok = Value;
    type Error = InvalidValue;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), InvalidValue>
    where
        T: Serialize,
    {
        match key.serialize(ValueSerializer) {
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
            self.entries.insert(KeyRef::Value(key), transform(value));
        }
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        Ok(Value(ValueBuf::Map(
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
        if let Ok(key) = key.serialize(ValueSerializer) {
            self.entries.insert(KeyRef::Value(key), transform(value));
        }
        Ok(())
    }
}

pub struct SerializeStruct {
    fields: OwnedValueMap,
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
        self.fields.insert(KeyRef::Str(key), transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        Ok(ValueBuf::Map(Arc::new(self.fields), MapType::Normal).into())
    }
}

pub struct SerializeStructVariant {
    variant: &'static str,
    map: OwnedValueMap,
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
        self.map.insert(KeyRef::Str(key), transform(value));
        Ok(())
    }

    fn end(self) -> Result<Value, InvalidValue> {
        let mut rv = BTreeMap::new();
        rv.insert(
            self.variant,
            Value::from(ValueBuf::Map(Arc::new(self.map), MapType::Normal)),
        );
        Ok(rv.into())
    }
}
