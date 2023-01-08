use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::error::{Error, ErrorKind};
use crate::value::{Object, ObjectKind, StructObject, Value};
use crate::vm::state::State;

pub(crate) struct Loop {
    pub len: usize,
    pub idx: AtomicUsize,
    pub depth: usize,
    pub last_changed_value: Mutex<Option<Vec<Value>>>,
}

impl fmt::Debug for Loop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Loop");
        for attr in self.static_fields().unwrap() {
            s.field(attr, &self.get_field(attr).unwrap());
        }
        s.finish()
    }
}

impl Object for Loop {
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Struct(self)
    }

    fn call(&self, _state: &State, _args: &[Value]) -> Result<Value, Error> {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "loop cannot be called if reassigned to different variable",
        ))
    }

    fn call_method(&self, _state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        if name == "changed" {
            let mut last_changed_value = self.last_changed_value.lock().unwrap();
            let value = args.to_owned();
            let changed = last_changed_value.as_ref() != Some(&value);
            if changed {
                *last_changed_value = Some(value);
                Ok(Value::from(true))
            } else {
                Ok(Value::from(false))
            }
        } else if name == "cycle" {
            let idx = self.idx.load(Ordering::Relaxed);
            match args.get(idx % args.len()) {
                Some(arg) => Ok(arg.clone()),
                None => Ok(Value::UNDEFINED),
            }
        } else {
            Err(Error::new(
                ErrorKind::UnknownMethod,
                format!("loop object has no method named {}", name),
            ))
        }
    }
}

impl StructObject for Loop {
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
            ][..],
        )
    }

    fn get_field(&self, name: &str) -> Option<Value> {
        let idx = self.idx.load(Ordering::Relaxed) as u64;
        // if we never iterated, then all attributes are undefined.
        // this can happen in some rare circumstances where the engine
        // did not manage to iterate
        if idx == !0 {
            return Some(Value::UNDEFINED);
        }
        let len = self.len as u64;
        match name {
            "index0" => Some(Value::from(idx)),
            "index" => Some(Value::from(idx + 1)),
            "length" => Some(Value::from(len)),
            "revindex" => Some(Value::from(len.saturating_sub(idx))),
            "revindex0" => Some(Value::from(len.saturating_sub(idx).saturating_sub(1))),
            "first" => Some(Value::from(idx == 0)),
            "last" => Some(Value::from(len == 0 || idx == len - 1)),
            "depth" => Some(Value::from(self.depth + 1)),
            "depth0" => Some(Value::from(self.depth)),
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
