use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::Serialize;

use crate::value::{intern, Value};

/// Internal abstraction over keys
#[derive(Clone)]
pub enum KeyRef<'a> {
    /// The key is a value
    Value(Value),
    /// The key is a string slice
    Str(&'a str),
}

impl<'a> KeyRef<'a> {
    /// If this is a str, return it.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            KeyRef::Value(v) => v.as_str(),
            KeyRef::Str(s) => Some(s),
        }
    }

    /// If this is an i64 return it
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            KeyRef::Value(v) => i64::try_from(v.clone()).ok(),
            KeyRef::Str(_) => None,
        }
    }

    /// Return this as value.
    pub fn as_value(&self) -> Value {
        match self {
            KeyRef::Value(v) => v.clone(),
            KeyRef::Str(s) => Value::from(intern(s)),
        }
    }
}

impl<'a> Serialize for KeyRef<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            KeyRef::Value(v) => v.serialize(serializer),
            KeyRef::Str(s) => s.serialize(serializer),
        }
    }
}

impl<'a> fmt::Debug for KeyRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(v) => fmt::Debug::fmt(v, f),
            Self::Str(v) => fmt::Debug::fmt(v, f),
        }
    }
}

impl<'a> PartialEq for KeyRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(a), Some(b)) = (self.as_str(), other.as_str()) {
            a.eq(b)
        } else {
            self.as_value().eq(&other.as_value())
        }
    }
}

impl<'a> Eq for KeyRef<'a> {}

impl<'a> PartialOrd for KeyRef<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for KeyRef<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        if let (Some(a), Some(b)) = (self.as_str(), other.as_str()) {
            a.cmp(b)
        } else {
            self.as_value().cmp(&other.as_value())
        }
    }
}

impl<'a> Hash for KeyRef<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(s) = self.as_str() {
            s.hash(state)
        } else {
            self.as_value().hash(state)
        }
    }
}

#[cfg(feature = "key_interning")]
pub mod key_interning {
    use crate::utils::OnDrop;

    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;
    use std::sync::Arc;

    thread_local! {
        static STRING_KEY_CACHE: RefCell<HashSet<Arc<str>>> = Default::default();
        static USE_STRING_KEY_CACHE: Cell<bool> = const { Cell::new(false) };
    }

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
    pub(crate) fn try_intern(s: &str) -> Arc<str> {
        // strings longer than 16 bytes are never interned or if we're at
        // depth 0.  (serialization code outside of internal serialization)
        // not checking for depth can cause a memory leak.
        if s.len() > 16 || !USE_STRING_KEY_CACHE.with(|flag| flag.get()) {
            return Arc::from(s);
        }

        STRING_KEY_CACHE.with(|cache| {
            let mut set = cache.borrow_mut();
            match set.get(s) {
                Some(stored) => stored.clone(),
                None => {
                    let rv: Arc<str> = Arc::from(s.to_string());
                    set.insert(rv.clone());
                    rv
                }
            }
        })
    }
}
