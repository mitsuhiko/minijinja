use std::collections::{BTreeMap, HashSet};
use std::fmt;

use crate::environment::Environment;
use crate::value::Value;
use crate::vm::loop_object::LoopState;

type Locals<'env> = BTreeMap<&'env str, Value>;

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
        m.entries(self.locals.iter());
        if let Some(LoopState {
            object: ref controller,
            ..
        }) = self.current_loop
        {
            m.entry(&"loop", controller);
        }
        if !self.ctx.is_undefined() {
            m.entries(self.ctx.iter_as_str_map());
        }
        m.finish()
    }
}

#[cfg_attr(feature = "internal_debug", derive(Debug))]
#[derive(Default)]
pub(crate) struct Stack {
    values: Vec<Value>,
}

impl Stack {
    pub fn push(&mut self, arg: Value) {
        self.values.push(arg);
    }

    pub fn pop(&mut self) -> Value {
        self.values.pop().expect("stack was empty")
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

    pub fn peek(&self) -> &Value {
        self.values.last().expect("stack was empty")
    }
}

impl From<Vec<Value>> for Stack {
    fn from(values: Vec<Value>) -> Stack {
        Stack { values }
    }
}

#[derive(Default)]
pub(crate) struct Context<'env> {
    stack: Vec<Frame<'env>>,
}

impl<'env> fmt::Debug for Context<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn dump<'a>(
            m: &mut std::fmt::DebugMap,
            seen: &mut HashSet<&'a str>,
            ctx: &'a Context<'a>,
        ) -> fmt::Result {
            for frame in ctx.stack.iter().rev() {
                for (key, value) in frame.locals.iter() {
                    if !seen.contains(key) {
                        seen.insert(*key);
                        m.entry(key, value);
                    }
                }

                if let Some(ref l) = frame.current_loop {
                    if l.with_loop_var && !seen.contains(&"loop") {
                        seen.insert("loop");
                        m.entry(&"loop", &l.object);
                    }
                }

                for (key, value) in frame.ctx.iter_as_str_map() {
                    if !seen.contains(key) {
                        seen.insert(key);
                        m.entry(&key, &value);
                    }
                }
            }
            Ok(())
        }

        let mut m = f.debug_map();
        let mut seen = HashSet::new();
        dump(&mut m, &mut seen, self)?;
        m.finish()
    }
}

impl<'env> Context<'env> {
    /// Creates a context
    pub fn new(frame: Frame<'env>) -> Context<'env> {
        Context { stack: vec![frame] }
    }

    /// Freezes the context.
    ///
    /// This implementation is not particularly beautiful and highly inefficient.
    /// Since it's only used for the debug support changing this is not too
    /// critical.
    #[cfg(feature = "debug")]
    pub fn freeze<'a>(&'a self, env: &'a Environment) -> Locals {
        let mut rv = Locals::new();

        rv.extend(env.globals.iter().map(|(k, v)| (*k, v.clone())));

        for frame in self.stack.iter().rev() {
            // look at locals first
            rv.extend(frame.locals.iter().map(|(k, v)| (*k, v.clone())));

            // if we are a loop, check if we are looking up the special loop var.
            if let Some(ref l) = frame.current_loop {
                if l.with_loop_var {
                    rv.insert("loop", Value::from_rc_object(l.object.clone()));
                }
            }

            rv.extend(frame.ctx.iter_as_str_map());
        }

        rv
    }

    /// Stores a variable in the context.
    pub fn store(&mut self, key: &'env str, value: Value) {
        self.stack
            .last_mut()
            .expect("cannot store on empty stack")
            .locals
            .insert(key, value);
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
                    return Some(Value::from_rc_object(l.object.clone()));
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

        if let Some(value) = env.get_global(key) {
            return Some(value);
        }

        None
    }

    /// Pushes a new layer.
    pub fn push_frame(&mut self, layer: Frame<'env>) {
        self.stack.push(layer);
    }

    /// Pops the topmost layer.
    pub fn pop_frame(&mut self) -> Frame {
        self.stack.pop().expect("pop from empty context stack")
    }

    /// Returns the current locals.
    pub fn current_locals(&mut self) -> &mut Locals<'env> {
        &mut self.stack.last_mut().expect("empty stack").locals
    }

    /// Returns the current innermost loop.
    pub fn current_loop(&mut self) -> Option<&mut LoopState> {
        self.stack
            .iter_mut()
            .rev()
            .filter_map(|x| x.current_loop.as_mut())
            .next()
    }
}
