use std::collections::{BTreeMap, HashMap};

use crate::utils::SealedMarker;
use crate::Value;

/// The non-serde version of the serialize trait.
///
/// When the `serde` feature is disabled the engine uses this trait
/// instead of the serde one.  It cannot be implemented by custom
/// types and it is only implemented for some basic values.
pub trait Serialize {
    #[doc(hidden)]
    fn to_value(&self, _: SealedMarker) -> Value;
}

impl Serialize for Value {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        self.clone()
    }
}

impl<S: Serialize + ?Sized> Serialize for &S {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        (**self).to_value(SealedMarker)
    }
}

impl Serialize for str {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        Value::from(self)
    }
}

impl Serialize for String {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        Value::from(self)
    }
}

impl Serialize for std::sync::Arc<str> {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        Value::from(self.clone())
    }
}

impl<T: Serialize> Serialize for [T] {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        Value::from(
            self.iter()
                .map(|x| x.to_value(SealedMarker))
                .collect::<Vec<_>>(),
        )
    }
}

impl<T: Serialize, const N: usize> Serialize for [T; N] {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        self.as_slice().to_value(SealedMarker)
    }
}

impl<T: Serialize> Serialize for Vec<T> {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        self.as_slice().to_value(SealedMarker)
    }
}

impl<K: Serialize, V: Serialize> Serialize for HashMap<K, V> {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        Value::from(
            self.iter()
                .map(|(a, b)| (a.to_value(SealedMarker), b.to_value(SealedMarker)))
                .collect::<BTreeMap<_, _>>(),
        )
    }
}

impl<K: Serialize, V: Serialize> Serialize for BTreeMap<K, V> {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        Value::from(
            self.iter()
                .map(|(a, b)| (a.to_value(SealedMarker), b.to_value(SealedMarker)))
                .collect::<BTreeMap<_, _>>(),
        )
    }
}

impl<T: Serialize> Serialize for Option<T> {
    #[inline(always)]
    fn to_value(&self, _: SealedMarker) -> Value {
        match self {
            Some(value) => value.to_value(SealedMarker),
            None => Value::from(()),
        }
    }
}

macro_rules! impl_tuple {
    ($($name:ident),+) => {
        impl<$($name: Serialize),+> Serialize for ($($name,)+) {
            #[allow(non_snake_case)]
            fn to_value(&self, _: SealedMarker) -> Value {
                let ($($name,)+) = self;
                Value::from(super::Tuple::from(vec![
                    $($name.to_value(SealedMarker),)+
                ]))
            }
        }
    };
}

impl_tuple!(A);
impl_tuple!(A, B);
impl_tuple!(A, B, C);
impl_tuple!(A, B, C, D);
impl_tuple!(A, B, C, D, E);
impl_tuple!(A, B, C, D, E, F);
impl_tuple!(A, B, C, D, E, F, G);
impl_tuple!(A, B, C, D, E, F, G, H);
impl_tuple!(A, B, C, D, E, F, G, H, I);
impl_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

macro_rules! impl_primitive {
    ($ty:ty) => {
        impl Serialize for $ty {
            #[inline(always)]
            fn to_value(&self, _: SealedMarker) -> Value {
                Value::from(*self)
            }
        }
    };
}

impl_primitive!(());
impl_primitive!(bool);
impl_primitive!(char);
impl_primitive!(usize);
impl_primitive!(isize);
impl_primitive!(i8);
impl_primitive!(i16);
impl_primitive!(i32);
impl_primitive!(i64);
impl_primitive!(i128);
impl_primitive!(u8);
impl_primitive!(u16);
impl_primitive!(u32);
impl_primitive!(u64);
impl_primitive!(u128);
impl_primitive!(f32);
impl_primitive!(f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_fallback_serialization() {
        let value = Value::from_serialize([Some(1u32), None, Some(3)]);
        assert_eq!(value.to_string(), "[1, None, 3]");

        let value = Value::from_serialize(BTreeMap::from([("key", vec![1, 2, 3])]));
        assert_eq!(value.to_string(), r#"{"key": [1, 2, 3]}"#);

        let value = Value::from_serialize(("key", 42));
        assert_eq!(value.to_string(), r#"("key", 42)"#);
    }
}
