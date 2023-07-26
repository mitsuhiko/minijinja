use crate::value::ValueBox;

#[cfg(feature = "preserve_order")]
pub(crate) type ValueBoxMap<'a> = indexmap::IndexMap<ValueBox, ValueBox>;

#[cfg(not(feature = "preserve_order"))]
pub(crate) type ValueBoxMap<'a> = std::collections::BTreeMap<ValueBox, ValueBox>;

pub(crate) type OwnedValueBoxMap = ValueBoxMap<'static>;

#[inline(always)]
pub(crate) fn value_map_with_capacity(capacity: usize) -> OwnedValueBoxMap {
    #[cfg(not(feature = "preserve_order"))]
    {
        let _ = capacity;
        OwnedValueBoxMap::new()
    }
    #[cfg(feature = "preserve_order")]
    {
        OwnedValueBoxMap::with_capacity(crate::utils::untrusted_size_hint(capacity))
    }
}
