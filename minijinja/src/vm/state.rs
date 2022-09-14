use std::collections::BTreeMap;
use std::fmt;

use crate::compiler::instructions::Instructions;
use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::output::Output;
use crate::value::Value;
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
pub struct State<'vm, 'env, 'out, 'buf> {
    pub(crate) env: &'env Environment<'env>,
    pub(crate) ctx: Context<'env, 'vm>,
    pub(crate) current_block: Option<&'env str>,
    pub(crate) out: &'out mut Output<'buf>,
    pub(crate) instructions: &'vm Instructions<'env>,
    pub(crate) blocks: BTreeMap<&'env str, Vec<&'vm Instructions<'env>>>,
}

impl<'vm, 'env, 'out, 'buf> fmt::Debug for State<'vm, 'env, 'out, 'buf> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("State");
        ds.field("name", &self.instructions.name());
        ds.field("current_block", &self.current_block);
        ds.field("auto_escape", &self.out.auto_escape);
        ds.field("ctx", &self.ctx);
        ds.field("env", &self.env);
        ds.finish()
    }
}

impl<'vm, 'env, 'out, 'buf> State<'vm, 'env, 'out, 'buf> {
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
        self.out.auto_escape
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
            out: &mut Output::null(),
            instructions: &Instructions::new("<unknown>", ""),
            blocks: BTreeMap::default(),
        })
    }

    pub(crate) fn apply_filter(&self, name: &str, args: &[Value]) -> Result<Value, Error> {
        if let Some(filter) = self.env.get_filter(name) {
            filter.apply_to(self, args)
        } else {
            Err(Error::new(
                ErrorKind::UnknownFilter,
                format!("filter {} is unknown", name),
            ))
        }
    }

    pub(crate) fn perform_test(&self, name: &str, args: &[Value]) -> Result<bool, Error> {
        if let Some(test) = self.env.get_test(name) {
            test.perform(self, args)
        } else {
            Err(Error::new(
                ErrorKind::UnknownTest,
                format!("test {} is unknown", name),
            ))
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn make_debug_info(
        &self,
        pc: usize,
        instructions: &Instructions<'_>,
    ) -> crate::error::DebugInfo {
        let referenced_names = instructions.get_referenced_names(pc);
        crate::error::DebugInfo {
            template_source: Some(instructions.source().to_string()),
            context: Some(Value::from(self.ctx.freeze(self.env))),
            referenced_names: Some(referenced_names.iter().map(|x| x.to_string()).collect()),
        }
    }
}
