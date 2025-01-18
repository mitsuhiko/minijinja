use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{Error, ErrorKind};
use crate::value::{Enumerator, Object, Value, ValueIter};
use crate::vm::state::State;

pub(crate) struct LoopState {
    pub(crate) with_loop_var: bool,
    pub(crate) recurse_jump_target: Option<usize>,

    // if we're popping the frame, do we want to jump somewhere?  The
    // first item is the target jump instruction, the second argument
    // tells us if we need to end capturing.
    pub(crate) current_recursion_jump: Option<(usize, bool)>,

    // Depending on if adjacent_loop_items is enabled or not, the iterator
    // is stored either on the loop state or in the loop object.  This is
    // done because when the feature is disabled, we can avoid using a mutex.
    pub(crate) object: Arc<Loop>,
    #[cfg(not(feature = "adjacent_loop_items"))]
    iter: crate::value::ValueIter,
}

impl LoopState {
    pub fn new(
        iter: ValueIter,
        depth: usize,
        with_loop_var: bool,
        recurse_jump_target: Option<usize>,
        current_recursion_jump: Option<(usize, bool)>,
    ) -> LoopState {
        // for an iterator where the lower and upper bound are matching we can
        // consider them to have ExactSizeIterator semantics.  We do however not
        // expect ExactSizeIterator bounds themselves to support iteration by
        // other means.
        let len = match iter.size_hint() {
            (lower, Some(upper)) if lower == upper => Some(lower),
            _ => None,
        };
        LoopState {
            with_loop_var,
            recurse_jump_target,
            current_recursion_jump,
            object: Arc::new(Loop {
                idx: AtomicUsize::new(!0usize),
                len,
                depth,
                #[cfg(feature = "adjacent_loop_items")]
                iter: Mutex::new(AdjacentLoopItemIterWrapper::new(iter)),
                last_changed_value: Mutex::default(),
            }),
            #[cfg(not(feature = "adjacent_loop_items"))]
            iter,
        }
    }

    pub fn did_not_iterate(&self) -> bool {
        self.object.idx.load(Ordering::Relaxed) == 0
    }

    pub fn next(&mut self) -> Option<Value> {
        self.object.idx.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "adjacent_loop_items")]
        {
            self.object.iter.lock().unwrap().next()
        }
        #[cfg(not(feature = "adjacent_loop_items"))]
        {
            self.iter.next()
        }
    }
}

#[cfg(feature = "adjacent_loop_items")]
pub(crate) struct AdjacentLoopItemIterWrapper {
    prev_item: Option<Value>,
    current_item: Option<Value>,
    next_item: Option<Value>,
    iter: ValueIter,
}

#[cfg(feature = "adjacent_loop_items")]
impl AdjacentLoopItemIterWrapper {
    pub fn new(iterator: ValueIter) -> AdjacentLoopItemIterWrapper {
        AdjacentLoopItemIterWrapper {
            prev_item: None,
            current_item: None,
            next_item: None,
            iter: iterator,
        }
    }

    pub fn next(&mut self) -> Option<Value> {
        self.prev_item = self.current_item.take();
        self.current_item = if let Some(ref next) = self.next_item.take() {
            Some(next.clone())
        } else {
            self.next_item = None;
            self.iter.next()
        };
        self.current_item.clone()
    }

    pub fn next_item(&mut self) -> Value {
        if let Some(ref next) = self.next_item {
            next.clone()
        } else {
            self.next_item = self.iter.next();
            self.next_item.clone().unwrap_or_default()
        }
    }

    pub fn prev_item(&self) -> Value {
        self.prev_item.clone().unwrap_or_default()
    }
}

pub(crate) struct Loop {
    pub len: Option<usize>,
    pub idx: AtomicUsize,
    pub depth: usize,
    #[cfg(feature = "adjacent_loop_items")]
    pub iter: Mutex<AdjacentLoopItemIterWrapper>,
    pub last_changed_value: Mutex<Option<Vec<Value>>>,
}

impl fmt::Debug for Loop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Loop")
            .field("len", &self.len)
            .field("idx", &self.idx)
            .field("depth", &self.depth)
            .finish()
    }
}

impl Object for Loop {
    fn call(self: &Arc<Self>, _state: &State, _args: &[Value]) -> Result<Value, Error> {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "loop cannot be called if reassigned to different variable",
        ))
    }

    fn call_method(
        self: &Arc<Self>,
        _state: &State,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
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
            Err(Error::from(ErrorKind::UnknownMethod))
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Str(&[
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
        ])
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let key = some!(key.as_str());
        let idx = self.idx.load(Ordering::Relaxed) as u64;
        // if we never iterated, then all attributes are undefined.
        // this can happen in some rare circumstances where the engine
        // did not manage to iterate
        if idx == !0 {
            return Some(Value::UNDEFINED);
        }
        let len = self.len.map(|x| x as u64);
        match key {
            "index0" => Some(Value::from(idx)),
            "index" => Some(Value::from(idx + 1)),
            "length" => Some(len.map(Value::from).unwrap_or(Value::UNDEFINED)),
            "revindex" => Some(
                len.map(|len| Value::from(len.saturating_sub(idx)))
                    .unwrap_or(Value::UNDEFINED),
            ),
            "revindex0" => Some(
                len.map(|len| Value::from(len.saturating_sub(idx).saturating_sub(1)))
                    .unwrap_or(Value::UNDEFINED),
            ),
            "first" => Some(Value::from(idx == 0)),
            "last" => Some(len.map_or(Value::from(false), |len| {
                Value::from(len == 0 || idx == len - 1)
            })),
            "depth" => Some(Value::from(self.depth + 1)),
            "depth0" => Some(Value::from(self.depth)),
            #[cfg(feature = "adjacent_loop_items")]
            "previtem" => Some(self.iter.lock().unwrap().prev_item()),
            #[cfg(feature = "adjacent_loop_items")]
            "nextitem" => Some(self.iter.lock().unwrap().next_item()),
            _ => None,
        }
    }

    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<loop {}/{}>",
            self.idx.load(Ordering::Relaxed),
            match self.len {
                Some(ref len) => len as &dyn fmt::Display,
                None => &"?" as &dyn fmt::Display,
            },
        )
    }
}
