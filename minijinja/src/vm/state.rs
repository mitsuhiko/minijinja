use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::compiler::instructions::Instructions;
use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::utils::{AutoEscape, UndefinedBehavior};
use crate::value::{ArgType, Value};
use crate::vm::context::Context;

#[cfg(feature = "fuel")]
use crate::vm::fuel::FuelTracker;

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
    pub(crate) current_call: Option<&'env str>,
    pub(crate) auto_escape: AutoEscape,
    pub(crate) instructions: &'vm Instructions<'env>,
    pub(crate) blocks: BTreeMap<&'env str, BlockStack<'vm, 'env>>,
    #[allow(unused)]
    pub(crate) loaded_templates: BTreeSet<&'env str>,
    #[cfg(feature = "macros")]
    pub(crate) macros: std::sync::Arc<Vec<(&'vm Instructions<'env>, usize)>>,
    #[cfg(feature = "fuel")]
    pub(crate) fuel_tracker: Option<std::sync::Arc<FuelTracker>>,
}

impl<'vm, 'env> fmt::Debug for State<'vm, 'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("State");
        ds.field("name", &self.instructions.name());
        ds.field("current_block", &self.current_block);
        ds.field("current_call", &self.current_call);
        ds.field("auto_escape", &self.auto_escape);
        ds.field("ctx", &self.ctx);
        ds.field("env", &self.env);
        ds.finish()
    }
}

impl<'vm, 'env> State<'vm, 'env> {
    /// Returns a reference to the current environment.
    #[inline(always)]
    pub fn env(&self) -> &Environment<'_> {
        self.env
    }

    /// Returns the name of the current template.
    pub fn name(&self) -> &str {
        self.instructions.name()
    }

    /// Returns the current value of the auto escape flag.
    #[inline(always)]
    pub fn auto_escape(&self) -> AutoEscape {
        self.auto_escape
    }

    /// Returns the current undefined behavior.
    #[inline(always)]
    pub fn undefined_behavior(&self) -> UndefinedBehavior {
        self.env.undefined_behavior()
    }

    /// Returns the name of the innermost block.
    #[inline(always)]
    pub fn current_block(&self) -> Option<&str> {
        self.current_block
    }

    /// Returns the name of the item (filter, function, test, method) currently
    /// being called.
    #[inline(always)]
    pub fn current_call(&self) -> Option<&str> {
        self.current_call
    }

    /// Looks up a variable by name in the context.
    #[inline(always)]
    pub fn lookup(&self, name: &str) -> Option<Value> {
        self.ctx.load(self.env, name)
    }

    #[cfg(any(test, feature = "testutils"))]
    pub(crate) fn with_dummy<R, F: FnOnce(&State) -> R>(env: &'env Environment<'env>, f: F) -> R {
        f(&State {
            env,
            ctx: Context::default(),
            current_block: None,
            auto_escape: AutoEscape::None,
            instructions: &Instructions::new("<unknown>", ""),
            blocks: BTreeMap::new(),
            loaded_templates: BTreeSet::new(),
            macros: Default::default(),
            current_call: None,
            #[cfg(feature = "fuel")]
            fuel_tracker: env.fuel().map(FuelTracker::new),
        })
    }

    #[cfg(feature = "debug")]
    pub(crate) fn make_debug_info(
        &self,
        pc: usize,
        instructions: &Instructions<'_>,
    ) -> crate::debug::DebugInfo {
        crate::debug::DebugInfo {
            template_source: Some(instructions.source().to_string()),
            referenced_locals: instructions
                .get_referenced_names(pc)
                .into_iter()
                .filter_map(|n| Some((n.to_string(), some!(self.lookup(n)))))
                .collect(),
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
        self.instructions.get(self.depth).copied().unwrap()
    }

    pub fn push(&mut self) -> bool {
        if self.depth + 1 < self.instructions.len() {
            self.depth += 1;
            true
        } else {
            false
        }
    }

    #[track_caller]
    pub fn pop(&mut self) {
        self.depth = self.depth.checked_sub(1).unwrap()
    }

    #[cfg(feature = "multi_template")]
    pub fn append_instructions(&mut self, instructions: &'vm Instructions<'env>) {
        self.instructions.push(instructions);
    }
}
