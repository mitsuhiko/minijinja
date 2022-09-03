use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::TryFromIntError;

use crate::error::{Error, ErrorKind};
use crate::value::{RcType, Value, ValueRepr};

pub use crate::key::serialize::KeySerializer;

#[cfg(feature = "deserialization")]
mod deserialize;
mod serialize;

/// Represents a key in a value's map.
#[derive(Clone)]
pub enum Key<'a> {
    Bool(bool),
    I64(i64),
    Char(char),
    String(RcType<String>),
    Str(&'a str),
}

pub type StaticKey = Key<'static>;

impl<'a> fmt::Debug for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(val) => fmt::Debug::fmt(val, f),
            Self::I64(val) => fmt::Debug::fmt(val, f),
            Self::Char(val) => fmt::Debug::fmt(val, f),
            Self::String(val) => fmt::Debug::fmt(val, f),
            Self::Str(val) => fmt::Debug::fmt(val, f),
        }
    }
}

#[derive(PartialOrd, Ord, Eq, PartialEq, Hash)]
enum InternalKeyRef<'a> {
    Bool(bool),
    I64(i64),
    Char(char),
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
            Key::String(RcType::new(String::from(s)))
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
            Key::Char(x) => InternalKeyRef::Char(x),
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
            ValueRepr::U128(ref v) => TryFrom::try_from(**v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::I64(v) => Ok(Key::I64(v)),
            ValueRepr::I128(ref v) => TryFrom::try_from(**v)
                .map(Key::I64)
                .map_err(|_| ErrorKind::NonKey.into()),
            ValueRepr::F64(x) => {
                // if a float is in fact looking like an integer we
                // allow this to be used for indexing.  Why?  Because
                // in Jinja division is always a division resuling
                // in floating point values (4 / 2 == 2.0).
                let intval = x as i64;
                if intval as f64 == x {
                    Ok(Key::I64(intval))
                } else {
                    Err(ErrorKind::NonKey.into())
                }
            }
            ValueRepr::Char(c) => Ok(Key::Char(c)),
            ValueRepr::String(ref s) => Ok(Key::Str(s)),
            _ => Err(ErrorKind::NonKey.into()),
        }
    }
}

impl<'a> PartialEq for Key<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.as_key_ref().eq(&other.as_key_ref())
    }
}

impl<'a> Eq for Key<'a> {}

impl<'a> Hash for Key<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_key_ref().hash(state)
    }
}

impl<'a> PartialOrd for Key<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_key_ref().partial_cmp(&other.as_key_ref())
    }
}

impl<'a> Ord for Key<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_key_ref().cmp(&other.as_key_ref())
    }
}

impl<'a> fmt::Display for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Bool(val) => write!(f, "{}", val),
            Key::I64(val) => write!(f, "{}", val),
            Key::Char(val) => write!(f, "{}", val),
            Key::String(val) => write!(f, "{}", val),
            Key::Str(val) => write!(f, "{}", val),
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
key_from!(char, Char);

impl TryFrom<u64> for StaticKey {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        TryFrom::try_from(value).map(Key::I64)
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
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::utils::OnDrop;

    thread_local! {
        static STRING_KEY_CACHE: RefCell<HashSet<CachedKey<'static>>> = Default::default();
        static STRING_KEY_CACHE_DEPTH: AtomicUsize = AtomicUsize::new(0);
    }

    enum CachedKey<'a> {
        Ref(&'a str),
        Stored(RcType<String>),
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

    pub(crate) fn with<R, F: FnOnce() -> R>(f: F) -> R {
        STRING_KEY_CACHE.with(|cache| {
            STRING_KEY_CACHE_DEPTH.with(|depth| {
                depth.fetch_add(1, Ordering::Relaxed);
                let _on_drop = OnDrop::new(|| {
                    if depth.fetch_sub(1, Ordering::Relaxed) == 1 {
                        cache.borrow_mut().clear();
                    }
                });
                f()
            })
        })
    }

    pub(crate) fn try_intern(s: &str) -> RcType<String> {
        let depth = STRING_KEY_CACHE_DEPTH.with(|depth| depth.load(Ordering::Relaxed));

        // strings longer than 16 bytes are never interned or if we're at
        // depth 0.  (serialization code outside of internal serialization)
        // not checking for depth can cause a memory leak.
        if depth == 0 || s.len() > 16 {
            return RcType::new(String::from(s));
        }

        STRING_KEY_CACHE.with(|cache| {
            let mut set = cache.borrow_mut();
            match set.get(&CachedKey::Ref(s)) {
                Some(CachedKey::Stored(s)) => s.clone(),
                None => {
                    let rv = RcType::new(String::from(s));
                    set.insert(CachedKey::Stored(rv.clone()));
                    rv
                }
                _ => unreachable!(),
            }
        })
    }
}
