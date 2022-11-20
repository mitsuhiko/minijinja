use std::borrow::Cow;
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
        for attr in self.fields() {
            s.field(&attr, &self.get_field(&attr).unwrap());
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
    fn fields(&self) -> Box<dyn Iterator<Item = Cow<'static, str>> + '_> {
        Box::new(
            [
                Cow::Borrowed("index0"),
                Cow::Borrowed("index"),
                Cow::Borrowed("length"),
                Cow::Borrowed("revindex"),
                Cow::Borrowed("revindex0"),
                Cow::Borrowed("first"),
                Cow::Borrowed("last"),
                Cow::Borrowed("depth"),
                Cow::Borrowed("depth0"),
            ]
            .into_iter(),
        )
    }

    fn get_field(&self, name: &str) -> Option<Value> {
        let idx = self.idx.load(Ordering::Relaxed) as u64;
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
