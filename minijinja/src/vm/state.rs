use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::compiler::instructions::Instructions;
use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::value::{ArgType, Value};
use crate::vm::context::Context;
use crate::AutoEscape;

/// Provides access to the current execution state of the engine.
///
/// A read only reference is passed to filter functions and similar objects to
/// allow limited interfacing with the engine.  The state is useful to look up
/// information about the engine in filter, test or global functions.  It not
/// only provides access to the template environment but also the context
/// variables of the engine, the current auto escaping behavior as well as the
/// auto escape flag.
///
/// **Notes on lifetimes:** the state object exposes some of the internal
/// lifetimes through the type.  You should always elide these lifetimes
/// as there might be lifetimes added or removed between releases.
pub struct State<'vm, 'env> {
    pub(crate) env: &'env Environment<'env>,
    pub(crate) ctx: Context<'env>,
    pub(crate) current_block: Option<&'env str>,
    pub(crate) auto_escape: AutoEscape,
    pub(crate) instructions: &'vm Instructions<'env>,
    pub(crate) blocks: BTreeMap<&'env str, BlockStack<'vm, 'env>>,
    pub(crate) macros: Arc<Vec<(&'vm Instructions<'env>, usize)>>,
}

impl<'vm, 'env> fmt::Debug for State<'vm, 'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("State");
        ds.field("name", &self.instructions.name());
        ds.field("current_block", &self.current_block);
        ds.field("auto_escape", &self.auto_escape);
        ds.field("ctx", &self.ctx);
        ds.field("env", &self.env);
        ds.finish()
    }
}

impl<'vm, 'env> State<'vm, 'env> {
    /// Returns a reference to the current environment.
    pub fn env(&self) -> &Environment<'_> {
        self.env
    }

    /// Returns the name of the current template.
    pub fn name(&self) -> &str {
        self.instructions.name()
    }

    /// Returns the current value of the auto escape flag.
    pub fn auto_escape(&self) -> AutoEscape {
        self.auto_escape
    }

    /// Returns the name of the innermost block.
    pub fn current_block(&self) -> Option<&str> {
        self.current_block
    }

    /// Looks up a variable by name in the context.
    pub fn lookup(&self, name: &str) -> Option<Value> {
        self.ctx.load(self.env(), name)
    }

    #[cfg(test)]
    pub(crate) fn with_dummy<R, F: FnOnce(&State) -> R>(env: &'env Environment<'env>, f: F) -> R {
        f(&State {
            env,
            ctx: Context::default(),
            current_block: None,
            auto_escape: AutoEscape::None,
            instructions: &Instructions::new("<unknown>", ""),
            blocks: BTreeMap::new(),
            macros: Arc::new(Vec::new()),
        })
    }

    #[cfg(feature = "debug")]
    pub(crate) fn make_debug_info(
        &self,
        pc: usize,
        instructions: &Instructions<'_>,
    ) -> crate::debug::DebugInfo {
        let referenced_names = instructions.get_referenced_names(pc);
        crate::debug::DebugInfo {
            template_source: Some(instructions.source().to_string()),
            context: Some(Value::from(self.ctx.freeze(self.env))),
            referenced_names: Some(referenced_names.iter().map(|x| x.to_string()).collect()),
        }
    }
}

impl<'a> ArgType<'a> for &State<'_, '_> {
    type Output = &'a State<'a, 'a>;

    fn from_value(_value: Option<&'a Value>) -> Result<Self::Output, Error> {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "cannot use state type in this position",
        ))
    }

    fn from_state_and_value(
        state: Option<&'a State>,
        _value: Option<&'a Value>,
    ) -> Result<(Self::Output, usize), Error> {
        match state {
            None => Err(Error::new(ErrorKind::InvalidOperation, "state unavailable")),
            Some(state) => Ok((state, 0)),
        }
    }
}

/// Tracks a block and it's parents for super.
#[derive(Default)]
pub(crate) struct BlockStack<'vm, 'env> {
    instructions: Vec<&'vm Instructions<'env>>,
    depth: usize,
}

impl<'vm, 'env> BlockStack<'vm, 'env> {
    pub fn new(instructions: &'vm Instructions<'env>) -> BlockStack<'vm, 'env> {
        BlockStack {
            instructions: vec![instructions],
            depth: 0,
        }
    }

    pub fn instructions(&self) -> &'vm Instructions<'env> {
        self.instructions
            .get(self.depth)
            .copied()
            .expect("block stack overflow")
    }

    pub fn push(&mut self) -> bool {
        if self.depth + 1 < self.instructions.len() {
            self.depth += 1;
            true
        } else {
            false
        }
    }

    pub fn pop(&mut self) {
        self.depth = self.depth.checked_sub(1).expect("block stack underflow");
    }

    pub fn append_instructions(&mut self, instructions: &'vm Instructions<'env>) {
        self.instructions.push(instructions);
    }
}
