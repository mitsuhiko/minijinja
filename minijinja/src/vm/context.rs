use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::value::{OwnedValueIterator, Value};
use crate::vm::loop_object::Loop;

type Locals<'env> = BTreeMap<&'env str, Value>;

/// The maximum recursion in the VM.  Normally each stack frame
/// adds one to this counter (eg: every time a frame is added).
/// However in some situations more depth is pushed if the cost
/// of the stack frame is higher.
const MAX_RECURSION: usize = 500;

pub(crate) struct LoopState {
    pub(crate) with_loop_var: bool,
    pub(crate) recurse_jump_target: Option<usize>,
    // if we're popping the frame, do we want to jump somewhere?  The
    // first item is the target jump instruction, the second argument
    // tells us if we need to end capturing.
    pub(crate) current_recursion_jump: Option<(usize, bool)>,
    pub(crate) iterator: OwnedValueIterator,
    pub(crate) object: Arc<Loop>,
}

pub(crate) struct Frame<'env> {
    pub(crate) locals: Locals<'env>,
    pub(crate) ctx: Value,
    pub(crate) current_loop: Option<LoopState>,
}

impl<'env> Default for Frame<'env> {
    fn default() -> Frame<'env> {
        Frame::new(Value::UNDEFINED)
    }
}

impl<'env> Frame<'env> {
    pub fn new(ctx: Value) -> Frame<'env> {
        Frame {
            locals: Locals::new(),
            ctx,
            current_loop: None,
        }
    }
}

#[cfg(feature = "internal_debug")]
impl<'env> fmt::Debug for Frame<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        m.entry(&"locals", &self.locals);
        if let Some(LoopState {
            object: ref controller,
            ..
        }) = self.current_loop
        {
            m.entry(&"loop", controller);
        }
        if !self.ctx.is_undefined() {
            m.entry(&"ctx", &self.ctx);
        }
        m.finish()
    }
}

#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub(crate) struct Stack {
    values: Vec<Value>,
}

impl Default for Stack {
    fn default() -> Stack {
        Stack {
            values: Vec::with_capacity(16),
        }
    }
}

impl Stack {
    pub fn push(&mut self, arg: Value) {
        self.values.push(arg);
    }

    #[track_caller]
    pub fn pop(&mut self) -> Value {
        self.values.pop().unwrap()
    }

    pub fn slice_top(&mut self, n: usize) -> &[Value] {
        &self.values[self.values.len() - n..]
    }

    pub fn drop_top(&mut self, n: usize) {
        self.values.truncate(self.values.len() - n);
    }

    pub fn try_pop(&mut self) -> Option<Value> {
        self.values.pop()
    }

    #[track_caller]
    pub fn peek(&self) -> &Value {
        self.values.last().unwrap()
    }
}

impl From<Vec<Value>> for Stack {
    fn from(values: Vec<Value>) -> Stack {
        Stack { values }
    }
}

pub(crate) struct Context<'env> {
    stack: Vec<Frame<'env>>,
    outer_stack_depth: usize,
}

impl<'env> fmt::Debug for Context<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn dump<'a>(
            m: &mut std::fmt::DebugMap,
            seen: &mut HashSet<Cow<'a, str>>,
            ctx: &'a Context<'a>,
        ) -> fmt::Result {
            for frame in ctx.stack.iter().rev() {
                for (key, value) in frame.locals.iter() {
                    if !seen.contains(&Cow::Borrowed(*key)) {
                        m.entry(&key, value);
                        seen.insert(Cow::Borrowed(key));
                    }
                }

                if let Some(ref l) = frame.current_loop {
                    if l.with_loop_var && !seen.contains("loop") {
                        m.entry(&"loop", &l.object);
                        seen.insert(Cow::Borrowed("loop"));
                    }
                }

                if let Ok(iter) = frame.ctx.try_iter() {
                    for key in iter {
                        if let Some(str_key) = key.as_str() {
                            if !seen.contains(&Cow::Borrowed(str_key)) {
                                if let Ok(value) = frame.ctx.get_item(&key) {
                                    m.entry(&str_key, &value);
                                    seen.insert(Cow::Owned(str_key.to_owned()));
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        let mut m = f.debug_map();
        let mut seen = HashSet::new();
        ok!(dump(&mut m, &mut seen, self));
        m.finish()
    }
}

impl<'env> Default for Context<'env> {
    fn default() -> Context<'env> {
        Context {
            stack: Vec::with_capacity(32),
            outer_stack_depth: 0,
        }
    }
}

impl<'env> Context<'env> {
    /// Creates a context
    pub fn new(frame: Frame<'env>) -> Context<'env> {
        let mut rv = Context::default();
        rv.stack.push(frame);
        rv
    }

    /// Stores a variable in the context.
    pub fn store(&mut self, key: &'env str, value: Value) {
        self.stack.last_mut().unwrap().locals.insert(key, value);
    }

    /// Looks up a variable in the context.
    pub fn load(&self, env: &Environment, key: &str) -> Option<Value> {
        for frame in self.stack.iter().rev() {
            // look at locals first
            if let Some(value) = frame.locals.get(key) {
                if !value.is_undefined() {
                    return Some(value.clone());
                }
            }

            // if we are a loop, check if we are looking up the special loop var.
            if let Some(ref l) = frame.current_loop {
                if l.with_loop_var && key == "loop" {
                    return Some(Value::from(l.object.clone()));
                }
            }

            // if the frame context is undefined, we skip the lookup
            if !frame.ctx.is_undefined() {
                if let Ok(rv) = frame.ctx.get_attr(key) {
                    if !rv.is_undefined() {
                        return Some(rv);
                    }
                }
            }
        }

        env.get_global(key)
    }

    /// Pushes a new layer.
    pub fn push_frame(&mut self, layer: Frame<'env>) -> Result<(), Error> {
        ok!(self.check_depth());
        self.stack.push(layer);
        Ok(())
    }

    /// Pops the topmost layer.
    #[track_caller]
    pub fn pop_frame(&mut self) -> Frame {
        self.stack.pop().unwrap()
    }

    /// Returns the current locals.
    #[track_caller]
    #[cfg(feature = "multi-template")]
    pub fn current_locals(&mut self) -> &mut Locals<'env> {
        &mut self.stack.last_mut().unwrap().locals
    }

    /// Returns the current innermost loop.
    pub fn current_loop(&mut self) -> Option<&mut LoopState> {
        self.stack
            .iter_mut()
            .rev()
            .filter_map(|x| x.current_loop.as_mut())
            .next()
    }

    /// The real depth of the context.
    pub fn depth(&self) -> usize {
        self.outer_stack_depth + self.stack.len()
    }

    /// Increase the stack depth.
    #[allow(unused)]
    pub fn incr_depth(&mut self, delta: usize) -> Result<(), Error> {
        self.outer_stack_depth += delta;
        ok!(self.check_depth());
        Ok(())
    }

    /// Decrease the stack depth.
    #[allow(unused)]
    pub fn decr_depth(&mut self, delta: usize) {
        self.outer_stack_depth -= delta;
    }

    fn check_depth(&self) -> Result<(), Error> {
        if self.depth() > MAX_RECURSION {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "recursion limit exceeded",
            ));
        }
        Ok(())
    }
}
