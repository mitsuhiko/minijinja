use crate::value::Value;

#[cfg(feature = "preserve_order")]
pub(crate) type ValueMap<'a> = indexmap::IndexMap<Value, Value>;

#[cfg(not(feature = "preserve_order"))]
pub(crate) type ValueMap<'a> = std::collections::BTreeMap<Value, Value>;

pub(crate) type OwnedValueMap = ValueMap<'static>;

#[inline(always)]
pub(crate) fn value_map_with_capacity(capacity: usize) -> OwnedValueMap {
    #[cfg(not(feature = "preserve_order"))]
    {
        let _ = capacity;
        OwnedValueMap::new()
    }
    #[cfg(feature = "preserve_order")]
    {
        OwnedValueMap::with_capacity(crate::utils::untrusted_size_hint(capacity))
    }
}
