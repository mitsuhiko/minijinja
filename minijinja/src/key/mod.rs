use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::TryFromIntError;
use std::sync::Arc;

use crate::error::{Error, ErrorKind};
use crate::value::{Value, ValueRepr};

pub use crate::key::serialize::KeySerializer;

#[cfg(feature = "deserialization")]
mod deserialize;
mod serialize;

/// Represents a key in a value's map.
#[derive(Clone)]
pub enum Key<'a> {
    Bool(bool),
    I64(i64),
    String(Arc<String>),
    Str(&'a str),
}

pub type StaticKey = Key<'static>;

impl<'a> fmt::Debug for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(val) => fmt::Debug::fmt(val, f),
            Self::I64(val) => fmt::Debug::fmt(val, f),
            Self::String(val) => fmt::Debug::fmt(val, f),
            Self::Str(val) => fmt::Debug::fmt(val, f),
        }
    }
}

#[derive(PartialOrd, Ord, Eq, PartialEq, Hash)]
enum InternalKeyRef<'a> {
    Bool(bool),
    I64(i64),
    Str(&'a str),
}

impl<'a> Key<'a> {
    pub fn make_string_key(s: &str) -> StaticKey {
        #[cfg(feature = "key_interning")]
        {
            Key::String(key_interning::try_intern(s))
        }
        #[cfg(not(feature = "key_interning"))]
        {
            Key::String(Arc::new(String::from(s)))
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match *self {
            Key::String(ref x) => Some(x.as_str()),
            Key::Str(x) => Some(x),
            _ => None,
        }
    }

    fn as_key_ref(&self) -> InternalKeyRef<'_> {
        match *self {
            Key::Bool(x) => InternalKeyRef::Bool(x),
            Key::I64(x) => InternalKeyRef::I64(x),
            Key::String(ref x) => InternalKeyRef::Str(x.as_str()),
            Key::Str(x) => InternalKeyRef::Str(x),
        }
    }

    pub fn from_borrowed_value(value: &'a Value) -> Result<Key<'a>, Error> {
        match value.0 {
            ValueRepr::Bool(v) => Ok(Key::Bool(v)),
            ValueRepr::U64(v) => TryFrom::try_from(v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::U128(v) => TryFrom::try_from(v.0)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::I64(v) => Ok(Key::I64(v)),
            ValueRepr::I128(v) => TryFrom::try_from(v.0)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::F64(x) => {
                // if a float is in fact looking like an integer we
                // allow this to be used for indexing.  Why?  Because
                // in Jinja division is always a division resulting
                // in floating point values (4 / 2 == 2.0).
                let intval = x as i64;
                if intval as f64 == x {
                    Ok(Key::I64(intval))
                } else {
                    Err(ErrorKind::NonKey.into())
                }
            }
            ValueRepr::String(ref s, _) => Ok(Key::Str(s)),
            _ => Err(ErrorKind::NonKey.into()),
        }
    }
}

impl<'a> PartialEq for Key<'a> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_key_ref().eq(&other.as_key_ref())
    }
}

impl<'a> Eq for Key<'a> {}

impl<'a> Hash for Key<'a> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_key_ref().hash(state)
    }
}

impl<'a> PartialOrd for Key<'a> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_key_ref().partial_cmp(&other.as_key_ref())
    }
}

impl<'a> Ord for Key<'a> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_key_ref().cmp(&other.as_key_ref())
    }
}

impl<'a> fmt::Display for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Bool(val) => write!(f, "{val}"),
            Key::I64(val) => write!(f, "{val}"),
            Key::String(val) => write!(f, "{val}"),
            Key::Str(val) => write!(f, "{val}"),
        }
    }
}

macro_rules! key_from {
    ($src:ty, $dst:ident) => {
        impl From<$src> for StaticKey {
            #[inline(always)]
            fn from(val: $src) -> Self {
                Key::$dst(val as _)
            }
        }
    };
}

key_from!(bool, Bool);
key_from!(u8, I64);
key_from!(u16, I64);
key_from!(u32, I64);
key_from!(i8, I64);
key_from!(i16, I64);
key_from!(i32, I64);
key_from!(i64, I64);

impl TryFrom<u64> for StaticKey {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        TryFrom::try_from(value).map(Key::I64)
    }
}

impl From<Arc<String>> for StaticKey {
    #[inline(always)]
    fn from(value: Arc<String>) -> Self {
        Key::String(value)
    }
}

impl From<String> for StaticKey {
    #[inline(always)]
    fn from(value: String) -> Self {
        Key::String(Arc::new(value))
    }
}

impl<'a> From<&'a str> for StaticKey {
    #[inline(always)]
    fn from(value: &'a str) -> Self {
        Key::make_string_key(value)
    }
}

#[cfg(feature = "key_interning")]
pub mod key_interning {
    use crate::utils::OnDrop;

    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;

    thread_local! {
        static STRING_KEY_CACHE: RefCell<HashSet<CachedKey<'static>>> = Default::default();
        static USE_STRING_KEY_CACHE: Cell<bool> = Cell::new(false);
    }

    enum CachedKey<'a> {
        Ref(&'a str),
        Stored(Arc<String>),
    }

    impl<'a> CachedKey<'a> {
        fn as_str(&self) -> &str {
            match self {
                CachedKey::Ref(x) => x,
                CachedKey::Stored(x) => x.as_str(),
            }
        }
    }

    impl<'a> Hash for CachedKey<'a> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.as_str().hash(state)
        }
    }

    impl<'a> PartialEq for CachedKey<'a> {
        fn eq(&self, other: &Self) -> bool {
            self.as_str().eq(other.as_str())
        }
    }

    impl<'a> Eq for CachedKey<'a> {}

    pub fn use_string_cache() -> impl Drop {
        let was_enabled = USE_STRING_KEY_CACHE.with(|flag| {
            let was_enabled = flag.get();
            flag.set(true);
            was_enabled
        });
        OnDrop::new(move || {
            if !was_enabled {
                USE_STRING_KEY_CACHE.with(|flag| flag.set(false));
                STRING_KEY_CACHE.with(|cache| cache.borrow_mut().clear());
            }
        })
    }

    #[inline(always)]
    pub(crate) fn try_intern(s: &str) -> Arc<String> {
        // strings longer than 16 bytes are never interned or if we're at
        // depth 0.  (serialization code outside of internal serialization)
        // not checking for depth can cause a memory leak.
        if s.len() > 16 || !USE_STRING_KEY_CACHE.with(|flag| flag.get()) {
            return Arc::new(String::from(s));
        }

        STRING_KEY_CACHE.with(|cache| {
            let mut set = cache.borrow_mut();
            match set.get(&CachedKey::Ref(s)) {
                Some(CachedKey::Stored(s)) => s.clone(),
                None => {
                    let rv = Arc::new(String::from(s));
                    set.insert(CachedKey::Stored(rv.clone()));
                    rv
                }
                _ => unreachable!(),
            }
        })
    }

    #[test]
    fn test_key_interning() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("x", 1u32);

        let v = Value::from_serializable(&vec![m.clone(), m.clone(), m.clone()]);

        for value in v.try_iter_owned().unwrap() {
            match value.0 {
                ValueRepr::Map(m, _) => {
                    let k = m.iter().next().unwrap().0;
                    match k {
                        Key::String(s) => {
                            assert_eq!(Arc::strong_count(s), 3);
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn test_string_key_lookup() {
    let mut m = std::collections::BTreeMap::new();
    m.insert(Key::String(Arc::new("foo".into())), Value::from(42));
    let m = Value::from(m);
    assert_eq!(m.get_item(&Value::from("foo")).unwrap(), Value::from(42));
}

#[test]
fn test_int_key_lookup() {
    let mut m = std::collections::BTreeMap::new();
    m.insert(Key::I64(42), Value::from(42));
    m.insert(Key::I64(23), Value::from(23));
    let m = Value::from(m);
    assert_eq!(m.get_item(&Value::from(42.0f32)).unwrap(), Value::from(42));
    assert_eq!(m.get_item(&Value::from(42u32)).unwrap(), Value::from(42));

    let s = Value::from(vec![42i32, 23]);
    assert_eq!(s.get_item(&Value::from(0.0f32)).unwrap(), Value::from(42));
    assert_eq!(s.get_item(&Value::from(0i32)).unwrap(), Value::from(42));
}
