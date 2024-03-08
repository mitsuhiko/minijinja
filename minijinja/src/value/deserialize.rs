use std::ops::{Deref, DerefMut};

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{forward_to_deserialize_any, Deserialize};

use crate::value::{ArgType, KeyRef, MapType, Value, ValueKind, ValueMap, ValueRepr};
use crate::{Error, ErrorKind};

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
        fn $name<E>(self, v: $ty) -> Result<Value, E>
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

    fn visit_none<E>(self) -> Result<Value, E>
    where
        E: de::Error,
    {
        Ok(Value::from(()))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Value, E>
    where
        E: de::Error,
    {
        Ok(Value::from(()))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut visitor: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut rv = Vec::<Value>::new();
        while let Some(e) = ok!(visitor.next_element()) {
            rv.push(e);
        }
        Ok(Value::from(rv))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut rv = ValueMap::default();
        while let Some((k, v)) = ok!(map.next_entry()) {
            rv.insert(KeyRef::Value(k), v);
        }
        Ok(Value(ValueRepr::Map(rv.into(), MapType::Normal)))
    }
}

/// Utility type to deserialize an argument.
///
/// This allows you to directly accept a type that implements [`Deserialize`] as an
/// argument to a filter or test.  The type dereferences into the inner type and
/// it also lets you move out the inner type.
///
/// ```rust
/// # use minijinja::Environment;
/// use std::path::PathBuf;
/// use minijinja::value::ViaDeserialize;
///
/// fn dirname(path: ViaDeserialize<PathBuf>) -> String {
///     match path.parent() {
///         Some(parent) => parent.display().to_string(),
///         None => "".to_string()
///     }
/// }
///
/// # let mut env = Environment::new();
/// env.add_filter("dirname", dirname);
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "deserialization")))]
pub struct ViaDeserialize<T>(pub T);

impl<'a, T: Deserialize<'a>> ArgType<'a> for ViaDeserialize<T> {
    type Output = Self;

    fn from_value(value: Option<&'a Value>) -> Result<Self, Error> {
        match value {
            Some(value) => T::deserialize(value).map(ViaDeserialize),
            None => Err(Error::from(ErrorKind::MissingArgument)),
        }
    }
}

impl<T> Deref for ViaDeserialize<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for ViaDeserialize<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

struct ValueDeserializer {
    value: Value,
}

impl ValueDeserializer {
    fn new(value: Value) -> ValueDeserializer {
        ValueDeserializer { value }
    }
}

impl<'de> de::Deserializer<'de> for ValueDeserializer {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value.0 {
            ValueRepr::Invalid(ref error) => Err(de::Error::custom(error)),
            ValueRepr::Bool(v) => visitor.visit_bool(v),
            ValueRepr::U64(v) => visitor.visit_u64(v),
            ValueRepr::I64(v) => visitor.visit_i64(v),
            ValueRepr::I128(v) => visitor.visit_i128(v.0),
            ValueRepr::U128(v) => visitor.visit_u128(v.0),
            ValueRepr::F64(v) => visitor.visit_f64(v),
            ValueRepr::String(ref v, _) => visitor.visit_str(v),
            ValueRepr::Undefined | ValueRepr::None => visitor.visit_unit(),
            ValueRepr::Bytes(ref v) => visitor.visit_bytes(v),
            ValueRepr::Seq(_) | ValueRepr::Map(..) | ValueRepr::Dynamic(_) => {
                if let Some(s) = self.value.as_seq() {
                    visitor.visit_seq(de::value::SeqDeserializer::new(
                        s.iter().map(ValueDeserializer::new),
                    ))
                } else if self.value.kind() == ValueKind::Map {
                    visitor.visit_map(de::value::MapDeserializer::new(
                        ok!(self.value.try_iter()).map(|k| {
                            (
                                ValueDeserializer::new(k.clone()),
                                ValueDeserializer::new(self.value.get_item(&k).unwrap_or_default()),
                            )
                        }),
                    ))
                } else {
                    Err(de::Error::invalid_type(
                        value_to_unexpected(&self.value),
                        &"supported value",
                    ))
                }
            }
        }
    }

    fn deserialize_option<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value.0 {
            ValueRepr::None | ValueRepr::Undefined => visitor.visit_unit(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_enum<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        let (variant, value) = match self.value.kind() {
            ValueKind::Map => {
                let mut iter = ok!(self.value.try_iter());
                let variant = match iter.next() {
                    Some(v) => v,
                    None => {
                        return Err(de::Error::invalid_value(
                            de::Unexpected::Map,
                            &"map with a single key",
                        ))
                    }
                };
                if iter.next().is_some() {
                    return Err(de::Error::invalid_value(
                        de::Unexpected::Map,
                        &"map with a single key",
                    ));
                }
                let val = self.value.get_item_opt(&variant);
                (variant, val)
            }
            ValueKind::String => (self.value, None),
            _ => {
                return Err(de::Error::invalid_type(
                    value_to_unexpected(&self.value),
                    &"string or map",
                ))
            }
        };

        visitor.visit_enum(EnumDeserializer { variant, value })
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit
        seq bytes byte_buf map unit_struct
        tuple_struct struct tuple ignored_any identifier newtype_struct
    }
}

impl<'de> de::IntoDeserializer<'de, Error> for ValueDeserializer {
    type Deserializer = ValueDeserializer;

    fn into_deserializer(self) -> ValueDeserializer {
        self
    }
}

struct EnumDeserializer {
    variant: Value,
    value: Option<Value>,
}

impl<'de> de::EnumAccess<'de> for EnumDeserializer {
    type Error = Error;
    type Variant = VariantDeserializer;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, VariantDeserializer), Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(ValueDeserializer::new(self.variant))
            .map(|v| (v, VariantDeserializer { value: self.value }))
    }
}

struct VariantDeserializer {
    value: Option<Value>,
}

impl<'de> de::VariantAccess<'de> for VariantDeserializer {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.value {
            Some(value) => de::Deserialize::deserialize(ValueDeserializer::new(value)),
            None => Ok(()),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => seed.deserialize(ValueDeserializer::new(value)),
            None => Err(de::Error::invalid_type(
                de::Unexpected::UnitVariant,
                &"newtype variant",
            )),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value.as_ref().and_then(|x| x.as_seq()) {
            Some(seq) => de::Deserializer::deserialize_any(
                de::value::SeqDeserializer::new(seq.iter().map(ValueDeserializer::new)),
                visitor,
            ),
            None => Err(de::Error::invalid_type(
                self.value
                    .as_ref()
                    .map_or(de::Unexpected::Unit, value_to_unexpected),
                &"tuple variant",
            )),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value.as_ref().map(|x| (x.kind(), x)) {
            Some((ValueKind::Map, val)) => de::Deserializer::deserialize_any(
                de::value::MapDeserializer::new(ok!(val.try_iter()).map(|k| {
                    (
                        ValueDeserializer::new(k.clone()),
                        ValueDeserializer::new(val.get_item(&k).unwrap_or_default()),
                    )
                })),
                visitor,
            ),
            _ => Err(de::Error::invalid_type(
                self.value
                    .as_ref()
                    .map_or(de::Unexpected::Unit, value_to_unexpected),
                &"struct variant",
            )),
        }
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "deserialization")))]
impl<'de> de::Deserializer<'de> for Value {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        ValueDeserializer::new(self).deserialize_any(visitor)
    }

    fn deserialize_option<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        ValueDeserializer::new(self).deserialize_option(visitor)
    }

    fn deserialize_enum<V: de::Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        ValueDeserializer::new(self).deserialize_enum(name, variants, visitor)
    }

    fn deserialize_newtype_struct<V: de::Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        ValueDeserializer::new(self).deserialize_newtype_struct(name, visitor)
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit
        seq bytes byte_buf map unit_struct
        tuple_struct struct tuple ignored_any identifier
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "deserialization")))]
impl<'de, 'v> de::Deserializer<'de> for &'v Value {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        ValueDeserializer::new(self.clone()).deserialize_any(visitor)
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit
        seq bytes byte_buf map unit_struct
        tuple_struct struct tuple ignored_any identifier
        option enum newtype_struct
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "deserialization")))]
impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::new(ErrorKind::CannotDeserialize, msg.to_string())
    }
}

fn value_to_unexpected(value: &Value) -> de::Unexpected {
    match value.0 {
        ValueRepr::Undefined | ValueRepr::None => de::Unexpected::Unit,
        ValueRepr::Bool(val) => de::Unexpected::Bool(val),
        ValueRepr::U64(val) => de::Unexpected::Unsigned(val),
        ValueRepr::I64(val) => de::Unexpected::Signed(val),
        ValueRepr::F64(val) => de::Unexpected::Float(val),
        ValueRepr::Invalid(_) => de::Unexpected::Other("<invalid value>"),
        ValueRepr::U128(val) => {
            let unsigned = val.0 as u64;
            if unsigned as u128 == val.0 {
                de::Unexpected::Unsigned(unsigned)
            } else {
                de::Unexpected::Other("u128")
            }
        }
        ValueRepr::I128(val) => {
            let signed = val.0 as i64;
            if signed as i128 == val.0 {
                de::Unexpected::Signed(signed)
            } else {
                de::Unexpected::Other("u128")
            }
        }
        ValueRepr::String(ref s, _) => de::Unexpected::Str(s),
        ValueRepr::Bytes(ref b) => de::Unexpected::Bytes(b),
        ValueRepr::Seq(_) => de::Unexpected::Seq,
        ValueRepr::Map(_, _) => de::Unexpected::Map,
        ValueRepr::Dynamic(ref d) => match d.kind() {
            super::ObjectKind::Plain => de::Unexpected::Other("plain object"),
            super::ObjectKind::Seq(_) => de::Unexpected::Seq,
            super::ObjectKind::Struct(_) => de::Unexpected::Map,
            super::ObjectKind::Iterator(_) => de::Unexpected::Other("iterator"),
        },
    }
}
