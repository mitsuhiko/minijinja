use std::collections::{BTreeMap, HashSet};
use std::fmt;

use crate::environment::Environment;
use crate::value::Value;
use crate::vm::forloop::ForLoop;

type Locals<'env> = BTreeMap<&'env str, Value>;

#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub(crate) enum FrameBase<'env, 'vm> {
    None,
    Context(&'vm Context<'env, 'vm>),
    Value(Value),
}

pub(crate) struct Frame<'env, 'vm> {
    pub(crate) locals: Locals<'env>,
    pub(crate) base: FrameBase<'env, 'vm>,
    pub(crate) current_loop: Option<ForLoop>,
}

impl<'env, 'vm> Default for Frame<'env, 'vm> {
    fn default() -> Frame<'env, 'vm> {
        Frame::new(FrameBase::None)
    }
}

impl<'env, 'vm> Frame<'env, 'vm> {
    pub fn new(base: FrameBase<'env, 'vm>) -> Frame<'env, 'vm> {
        Frame {
            locals: Locals::new(),
            base,
            current_loop: None,
        }
    }
}

#[cfg(feature = "internal_debug")]
impl<'env, 'vm> fmt::Debug for Frame<'env, 'vm> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        m.entries(self.locals.iter());
        if let Some(ForLoop {
            state: ref controller,
            ..
        }) = self.current_loop
        {
            m.entry(&"loop", controller);
        }
        if let FrameBase::Value(ref value) = self.base {
            m.entries(value.iter_as_str_map());
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

    pub fn try_pop(&mut self) -> Option<Value> {
        self.values.pop()
    }

    pub fn peek(&self) -> &Value {
        self.values.last().expect("stack was empty")
    }
}

#[derive(Default)]
pub(crate) struct Context<'env, 'vm> {
    stack: Vec<Frame<'env, 'vm>>,
}

impl<'env, 'vm> fmt::Debug for Context<'env, 'vm> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn dump<'a>(
            m: &mut std::fmt::DebugMap,
            seen: &mut HashSet<&'a str>,
            ctx: &'a Context<'a, 'a>,
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
                        m.entry(&"loop", &l.state);
                    }
                }

                match frame.base {
                    FrameBase::Context(ctx) => {
                        dump(m, seen, ctx)?;
                    }
                    FrameBase::Value(ref value) => {
                        for (key, value) in value.iter_as_str_map() {
                            if !seen.contains(key) {
                                seen.insert(key);
                                m.entry(&key, &value);
                            }
                        }
                    }
                    FrameBase::None => continue,
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

impl<'env, 'vm> Context<'env, 'vm> {
    /// Creates a context
    pub fn new(frame: Frame<'env, 'vm>) -> Context<'env, 'vm> {
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
                    rv.insert("loop", Value::from_rc_object(l.state.clone()));
                }
            }

            match frame.base {
                FrameBase::Context(ctx) => {
                    rv.extend(ctx.freeze(env));
                }
                FrameBase::Value(ref value) => {
                    rv.extend(value.iter_as_str_map());
                }
                FrameBase::None => continue,
            }
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
                    return Some(Value::from_rc_object(l.state.clone()));
                }
            }

            match frame.base {
                FrameBase::Context(ctx) => return ctx.load(env, key),
                FrameBase::Value(ref value) => {
                    let rv = value.get_attr(key);
                    if let Ok(rv) = rv {
                        if !rv.is_undefined() {
                            return Some(rv);
                        }
                    }
                    if let Some(value) = env.get_global(key) {
                        return Some(value);
                    }
                }
                FrameBase::None => continue,
            }
        }
        None
    }

    /// Pushes a new layer.
    pub fn push_frame(&mut self, layer: Frame<'env, 'vm>) {
        self.stack.push(layer);
    }

    /// Pops the topmost layer.
    pub fn pop_frame(&mut self) -> Frame {
        self.stack.pop().expect("pop from empty context stack")
    }

    /// Returns the current innermost loop.
    pub fn current_loop(&mut self) -> Option<&mut ForLoop> {
        self.stack
            .iter_mut()
            .rev()
            .filter_map(|x| x.current_loop.as_mut())
            .next()
    }
}
