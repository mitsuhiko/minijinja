use serde::de::{self, Visitor};
use serde::Deserialize;

use crate::key::{Key, StaticKey};

impl<'de> Deserialize<'de> for StaticKey {
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
    type Value = StaticKey;

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

    fn visit_char<E>(self, v: char) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Key::from(v.to_string()))
    }

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
