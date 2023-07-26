use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Arc};

use crate::error::{Error, ErrorKind};
use crate::value::{Object, MapObject, ValueBox, Value};
use crate::vm::state::State;

#[derive(Clone)]
pub(crate) struct Loop {
    pub status: Arc<LoopStatus>,
}

impl std::ops::Deref for Loop {
    type Target = LoopStatus;

    fn deref(&self) -> &Self::Target {
        &*self.status
    }
}

pub(crate) struct LoopStatus {
    pub len: usize,
    pub idx: AtomicUsize,
    pub depth: usize,
    #[cfg(feature = "adjacent_loop_items")]
    pub value_triple: Mutex<(Option<ValueBox>, Option<ValueBox>, Option<ValueBox>)>,
    pub last_changed_value: Mutex<Option<Vec<ValueBox>>>,
}

impl fmt::Debug for Loop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Loop");
        for attr in self.static_fields().unwrap() {
            s.field(attr, &self.get_field(&ValueBox::from(*attr)).unwrap());
        }
        s.finish()
    }
}

impl Object for Loop {
    fn value(&self) -> Value<'_> {
        Value::from_map_ref(self)
    }

    fn call(&self, _state: &State, _args: &[ValueBox]) -> Result<ValueBox, Error> {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "loop cannot be called if reassigned to different variable",
        ))
    }

    fn call_method(&self, _state: &State, name: &str, args: &[ValueBox]) -> Result<ValueBox, Error> {
        if name == "changed" {
            let mut last_changed_value = self.last_changed_value.lock().unwrap();
            let value = args.to_owned();
            let changed = last_changed_value.as_ref() != Some(&value);
            if changed {
                *last_changed_value = Some(value);
                Ok(ValueBox::from(true))
            } else {
                Ok(ValueBox::from(false))
            }
        } else if name == "cycle" {
            let idx = self.idx.load(Ordering::Relaxed);
            match args.get(idx % args.len()) {
                Some(arg) => Ok(arg.clone()),
                None => Ok(ValueBox::UNDEFINED),
            }
        } else {
            Err(Error::new(
                ErrorKind::UnknownMethod,
                format!("loop object has no method named {name}"),
            ))
        }
    }
}

impl MapObject for Loop {
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(
            &[
                "index0",
                "index",
                "length",
                "revindex",
                "revindex0",
                "first",
                "last",
                "depth",
                "depth0",
                #[cfg(feature = "adjacent_loop_items")]
                "previtem",
                #[cfg(feature = "adjacent_loop_items")]
                "nextitem",
            ][..],
        )
    }

    fn get_field(&self, key: &ValueBox) -> Option<ValueBox> {
        let name = key.as_str()?;
        let idx = self.idx.load(Ordering::Relaxed) as u64;
        // if we never iterated, then all attributes are undefined.
        // this can happen in some rare circumstances where the engine
        // did not manage to iterate
        if idx == !0 {
            return Some(ValueBox::UNDEFINED);
        }
        let len = self.len as u64;
        match name {
            "index0" => Some(ValueBox::from(idx)),
            "index" => Some(ValueBox::from(idx + 1)),
            "length" => Some(ValueBox::from(len)),
            "revindex" => Some(ValueBox::from(len.saturating_sub(idx))),
            "revindex0" => Some(ValueBox::from(len.saturating_sub(idx).saturating_sub(1))),
            "first" => Some(ValueBox::from(idx == 0)),
            "last" => Some(ValueBox::from(len == 0 || idx == len - 1)),
            "depth" => Some(ValueBox::from(self.depth + 1)),
            "depth0" => Some(ValueBox::from(self.depth)),
            #[cfg(feature = "adjacent_loop_items")]
            "previtem" => Some(
                self.value_triple
                    .lock()
                    .unwrap()
                    .0
                    .clone()
                    .unwrap_or(ValueBox::UNDEFINED),
            ),
            #[cfg(feature = "adjacent_loop_items")]
            "nextitem" => Some(
                self.value_triple
                    .lock()
                    .unwrap()
                    .2
                    .clone()
                    .unwrap_or(ValueBox::UNDEFINED),
            ),
            _ => None,
        }
    }
}

impl fmt::Display for Loop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<loop {}/{}>",
            self.idx.load(Ordering::Relaxed),
            self.len
        )
    }
}
