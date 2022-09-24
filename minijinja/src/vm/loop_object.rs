use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{Error, ErrorKind};
use crate::value::{Object, Value, ValueIterator};
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
        for attr in self.attributes() {
            s.field(attr, &self.get_attr(attr).unwrap());
        }
        s.finish()
    }
}

impl Object for Loop {
    fn attributes(&self) -> &[&str] {
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
        ][..]
    }

    fn get_attr(&self, name: &str) -> Option<Value> {
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
                ErrorKind::InvalidOperation,
                format!("loop object has no method named {}", name),
            ))
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

#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub(crate) struct LoopState {
    pub(crate) with_loop_var: bool,
    pub(crate) recurse_jump_target: Option<usize>,
    // if we're popping the frame, do we want to jump somewhere?  The
    // first item is the target jump instruction, the second argument
    // tells us if we need to end capturing.
    pub(crate) current_recursion_jump: Option<(usize, bool)>,
    pub(crate) iterator: ValueIterator,
    pub(crate) object: Arc<Loop>,
}
