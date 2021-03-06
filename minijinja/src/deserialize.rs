use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::key::Key;
use crate::value::{Value, ValueMap, ValueRepr};

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let visitor = ValueVisitor;
        deserializer.deserialize_any(visitor)
    }
}

struct ValueVisitor;

macro_rules! visit_value_primitive {
    ($name:ident, $ty:ty) => {
        fn $name<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Value::from(v))
        }
    };
}

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str("any MiniJinja compatible value")
    }

    visit_value_primitive!(visit_bool, bool);
    visit_value_primitive!(visit_i8, i8);
    visit_value_primitive!(visit_i16, i16);
    visit_value_primitive!(visit_i32, i32);
    visit_value_primitive!(visit_i64, i64);
    visit_value_primitive!(visit_i128, i128);
    visit_value_primitive!(visit_u16, u16);
    visit_value_primitive!(visit_u32, u32);
    visit_value_primitive!(visit_u64, u64);
    visit_value_primitive!(visit_u128, u128);
    visit_value_primitive!(visit_f32, f32);
    visit_value_primitive!(visit_f64, f64);
    visit_value_primitive!(visit_char, char);
    visit_value_primitive!(visit_str, &str);
    visit_value_primitive!(visit_string, String);
    visit_value_primitive!(visit_bytes, &[u8]);
    visit_value_primitive!(visit_byte_buf, Vec<u8>);

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::from(()))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::from(()))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut visitor: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut rv = Vec::<Value>::new();
        while let Some(e) = visitor.next_element()? {
            rv.push(e);
        }
        Ok(Value::from(rv))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut rv = ValueMap::default();
        while let Some((k, v)) = map.next_entry()? {
            rv.insert(k, v);
        }
        Ok(Value(ValueRepr::Map(rv.into())))
    }
}

impl<'de> Deserialize<'de> for Key<'static> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let visitor = KeyVisitor;
        deserializer.deserialize_any(visitor)
    }
}

struct KeyVisitor;

macro_rules! visit_key_primitive {
    ($name:ident, $ty:ty, $enum_ty:ident) => {
        fn $name<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Key::$enum_ty(v as _))
        }
    };
}

impl<'de> Visitor<'de> for KeyVisitor {
    type Value = Key<'static>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str("any MiniJinja compatible value")
    }

    visit_key_primitive!(visit_bool, bool, Bool);
    visit_key_primitive!(visit_i8, i8, I64);
    visit_key_primitive!(visit_i16, i16, I64);
    visit_key_primitive!(visit_i32, i32, I64);
    visit_key_primitive!(visit_i64, i64, I64);
    visit_key_primitive!(visit_u8, u8, I64);
    visit_key_primitive!(visit_u16, u16, I64);
    visit_key_primitive!(visit_u32, u32, I64);
    visit_key_primitive!(visit_u64, u64, I64);
    visit_key_primitive!(visit_char, char, Char);

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Key::make_string_key(v))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }
}
