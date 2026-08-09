use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::compiler::instructions::Instructions;
use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::output::Output;
use crate::template::Template;
use crate::utils::{AutoEscape, UndefinedBehavior};
use crate::value::Value;
use crate::vm::context::Context;

#[cfg(feature = "fuel")]
use crate::vm::fuel::FuelTracker;

/// When macros are used, the state carries an `id` counter.  Whenever a state is
/// created, the counter is incremented.  This exists because macros can keep a reference
/// to instructions from another state by index.  Without this counter it would
/// be possible for a macro to be called with a different state (different id)
/// which mean we likely panic.
#[cfg(feature = "macros")]
static STATE_ID: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Provides access to the current execution state of the engine.
///
/// The state is passed to filters, tests, functions, and callable objects to
/// let them interface with the engine.  Typed callbacks can request either a
/// shared or mutable reference.  Shared access is useful for inspecting the
/// template environment, context variables, and current auto-escaping behavior;
/// mutable access additionally permits nested calls and render-local storage.
///
/// In some testing scenarios or more advanced use cases you might need to get
/// a [`State`].  The state is managed as part of the template execution but the
/// initial state can be retrieved via [`Template::new_state`](crate::Template::new_state).
/// The most common way to get hold of the state however is via functions of filters.
///
/// **Notes on lifetimes:** the state object exposes some of the internal
/// lifetimes through the type.  You should always elide these lifetimes
/// as there might be lifetimes added or removed between releases.
pub struct State<'template, 'env> {
    pub(crate) ctx: Context<'env>,
    pub(crate) current_block: Option<&'env str>,
    pub(crate) auto_escape: AutoEscape,
    pub(crate) instructions: &'template Instructions<'env>,
    pub(crate) temps: BTreeMap<Box<str>, Value>,
    pub(crate) extensions: BTreeMap<TypeId, Box<dyn Any + Send>>,
    pub(crate) blocks: BTreeMap<&'env str, BlockStack<'template, 'env>>,
    #[allow(unused)]
    pub(crate) loaded_templates: BTreeSet<&'env str>,
    #[cfg(feature = "macros")]
    pub(crate) id: isize,
    #[cfg(feature = "macros")]
    pub(crate) macros: Vec<(&'template Instructions<'env>, u32)>,
    #[cfg(feature = "macros")]
    pub(crate) closures: Vec<crate::vm::closure_object::Closure>,
    #[cfg(feature = "macros")]
    pub(crate) macro_context_pool: Vec<Context<'env>>,
    #[cfg(feature = "fuel")]
    pub(crate) fuel_tracker: Option<FuelTracker>,
}

impl fmt::Debug for State<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("State");
        ds.field("name", &self.instructions.name());
        ds.field("current_block", &self.current_block);
        ds.field("auto_escape", &self.auto_escape);
        ds.field(
            "ctx",
            &self.ctx.debug(
                #[cfg(feature = "macros")]
                &self.closures,
            ),
        );
        ds.field("env", &self.env());
        ds.finish()
    }
}

impl<'template, 'env> State<'template, 'env> {
    /// Creates a new state.
    pub(crate) fn new(
        ctx: Context<'env>,
        auto_escape: AutoEscape,
        instructions: &'template Instructions<'env>,
        blocks: BTreeMap<&'env str, BlockStack<'template, 'env>>,
    ) -> State<'template, 'env> {
        State {
            #[cfg(feature = "macros")]
            id: STATE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            current_block: None,
            auto_escape,
            instructions,
            blocks,
            temps: Default::default(),
            extensions: Default::default(),
            loaded_templates: BTreeSet::new(),
            #[cfg(feature = "macros")]
            macros: Default::default(),
            #[cfg(feature = "macros")]
            closures: Default::default(),
            #[cfg(feature = "macros")]
            macro_context_pool: Default::default(),
            #[cfg(feature = "fuel")]
            fuel_tracker: ctx.env().fuel().map(FuelTracker::new),
            ctx,
        }
    }

    /// Creates an empty state for an environment.
    pub(crate) fn new_for_env(env: &'env Environment) -> State<'env, 'env> {
        State::new(
            Context::new(env),
            AutoEscape::None,
            &crate::compiler::instructions::EMPTY_INSTRUCTIONS,
            BTreeMap::new(),
        )
    }

    /// Returns a reference to the current environment.
    #[inline(always)]
    pub fn env(&self) -> &'env Environment<'env> {
        self.ctx.env()
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

    pub(crate) fn with_auto_escape<R>(
        &mut self,
        auto_escape: AutoEscape,
        f: impl FnOnce(&mut State<'template, 'env>) -> R,
    ) -> R {
        if self.auto_escape == auto_escape {
            return f(self);
        }

        let old = std::mem::replace(&mut self.auto_escape, auto_escape);
        let rv = f(self);
        self.auto_escape = old;
        rv
    }

    /// Returns the current undefined behavior.
    #[inline(always)]
    pub fn undefined_behavior(&self) -> UndefinedBehavior {
        self.env().undefined_behavior()
    }

    /// Returns the name of the innermost block.
    #[inline(always)]
    pub fn current_block(&self) -> Option<&str> {
        self.current_block
    }

    /// Looks up a variable by name in the context.
    ///
    /// # Note on Closures
    ///
    /// Macros and call blocks analyze which variables are referenced and
    /// create closures for them.  This means that unless a variable is defined
    /// as a [global](Environment::add_global) in the environment, was passed in the
    /// initial render context, or was referenced by a macro, this method won't be
    /// able to find it.
    #[inline(always)]
    pub fn lookup(&self, name: &str) -> Option<Value> {
        self.ctx.load(
            #[cfg(feature = "macros")]
            &self.closures,
            name,
        )
    }

    /// Looks up a global macro and calls it.
    ///
    /// This looks up a value as [`lookup`](Self::lookup) does and calls it
    /// with the passed args.
    #[cfg(feature = "macros")]
    #[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
    pub fn call_macro(&mut self, name: &str, args: &[Value]) -> Result<String, Error> {
        let f = ok!(self.lookup(name).ok_or_else(|| Error::new(
            crate::error::ErrorKind::UnknownFunction,
            "macro not found"
        )));
        f.call(self, args).map(Into::into)
    }

    /// Renders a block with the given name into a string.
    ///
    /// This method works like [`Template::render`](crate::Template::render) but
    /// it only renders a specific block in the template.  The first argument is
    /// the name of the block.
    ///
    /// This renders only the block `hi` in the template:
    ///
    /// ```
    /// # use minijinja::{Environment, context};
    /// # fn test() -> Result<(), minijinja::Error> {
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "{% block hi %}Hello {{ name }}!{% endblock %}")?;
    /// let tmpl = env.get_template("hello")?;
    /// let mut rendered = tmpl
    ///     .render_captured(context!(name => "John"))?;
    /// let rv = rendered.with_state_mut(|state| state.render_block("hi"))?;
    /// println!("{}", rv);
    /// # Ok(()) }
    /// ```
    ///
    /// Rendering a block is a stateful operation and therefore requires mutable
    /// access to the state.  Filters and functions can request this by taking
    /// `&mut State` as their first parameter.  Execution frames are restored if
    /// rendering fails, while explicit mutations to temps or extensions remain.
    #[cfg(feature = "multi_template")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multi_template")))]
    pub fn render_block(&mut self, block: &str) -> Result<String, Error> {
        let mut buf = String::new();
        crate::vm::Vm::call_block(block, self, &mut Output::new(&mut buf)).map(|_| buf)
    }

    /// Renders a block with the given name into an [`io::Write`](std::io::Write).
    ///
    /// For details see [`render_block`](Self::render_block).
    #[cfg(feature = "multi_template")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multi_template")))]
    pub fn render_block_to_write<W>(&mut self, block: &str, w: W) -> Result<(), Error>
    where
        W: std::io::Write,
    {
        let mut wrapper = crate::output::WriteWrapper { w, err: None };
        crate::vm::Vm::call_block(block, self, &mut Output::new(&mut wrapper))
            .map(|_| ())
            .map_err(|err| wrapper.take_err(err))
    }

    /// Returns a list of the names of all exports (top-level variables).
    pub fn exports(&self) -> Vec<&str> {
        self.ctx.exports().keys().copied().collect()
    }

    /// Returns a list of all known variables.
    ///
    /// This list contains all variables that are currently known to the state.
    /// To retrieve the values you can use [`lookup`](Self::lookup).  This will
    /// include all the globals of the environment.  Note that if the context
    /// has been initialized with an object that lies about variables (eg: it
    /// does not correctly implement enumeration), the returned list might not
    /// be complete.
    pub fn known_variables(&self) -> Vec<Cow<'_, str>> {
        Vec::from_iter(self.ctx.known_variables(
            #[cfg(feature = "macros")]
            &self.closures,
            true,
        ))
    }

    /// Fetches a template by name with path joining.
    ///
    /// This works like [`Environment::get_template`] with the difference that the lookup
    /// undergoes path joining.  If the environment has a configured path joining callback,
    /// it will be invoked with the name of the current template as parent template.
    ///
    /// For more information see [`Environment::set_path_join_callback`].
    pub fn get_template(&self, name: &str) -> Result<Template<'env, 'env>, Error> {
        self.env()
            .get_template(&self.env().join_template_path(name, self.name()))
    }

    /// Invokes a filter with some arguments.
    ///
    /// ```
    /// # use minijinja::Environment;
    /// # let mut env = Environment::new();
    /// # env.add_filter("upper", |x: &str| x.to_uppercase());
    /// # let tmpl = env.template_from_str("").unwrap();
    /// # let mut state = tmpl.new_state();
    /// let rv = state.apply_filter("upper", &["hello world".into()]).unwrap();
    /// assert_eq!(rv.as_str(), Some("HELLO WORLD"));
    /// ```
    pub fn apply_filter(&mut self, filter: &str, args: &[Value]) -> Result<Value, Error> {
        match self.env().get_filter(filter) {
            Some(filter) => filter.call(self, args),
            None => Err(Error::from(ErrorKind::UnknownFilter)),
        }
    }

    /// Invokes a test function on a value.
    ///
    /// ```
    /// # use minijinja::Environment;
    /// # let mut env = Environment::new();
    /// # env.add_test("even", |x: i32| x % 2 == 0);
    /// # let tmpl = env.template_from_str("").unwrap();
    /// # let mut state = tmpl.new_state();
    /// let rv = state.perform_test("even", &[42i32.into()]).unwrap();
    /// assert!(rv);
    /// ```
    pub fn perform_test(&mut self, test: &str, args: &[Value]) -> Result<bool, Error> {
        match self.env().get_test(test) {
            Some(test) => test.call(self, args).map(|x| x.is_true()),
            None => Err(Error::from(ErrorKind::UnknownTest)),
        }
    }

    /// Formats a value to a string using the formatter on the environment.
    ///
    /// ```
    /// # use minijinja::{value::Value, Environment};
    /// # let mut env = Environment::new();
    /// # let tmpl = env.template_from_str("").unwrap();
    /// # let mut state = tmpl.new_state();
    /// let rv = state.format(Value::from(42)).unwrap();
    /// assert_eq!(rv, "42");
    /// ```
    pub fn format(&mut self, value: Value) -> Result<String, Error> {
        let mut rv = String::new();
        let mut out = Output::new(&mut rv);
        self.env().format(&value, self, &mut out).map(|_| rv)
    }

    /// Returns the fuel levels.
    ///
    /// When the fuel feature is enabled, during evaluation the template will keep
    /// track of how much fuel it has consumed.  If the fuel tracker is turned on
    /// the returned value will be `Some((consumed, remaining))`.  If fuel tracking
    /// is not enabled, `None` is returned instead.
    #[cfg(feature = "fuel")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fuel")))]
    pub fn fuel_levels(&self) -> Option<(u64, u64)> {
        self.fuel_tracker
            .as_ref()
            .map(|x| (x.consumed(), x.remaining()))
    }

    /// Looks up a temp and returns it.
    ///
    /// Temps are similar to context values but the engine never looks them up
    /// on their own and they are not scoped.  The lifetime of temps is limited
    /// to the rendering process of a template.  Temps are useful so that
    /// filters and other things can temporarily stash away state without having
    /// to resort to thread locals which are hard to manage.  Unlike context
    /// variables, temps can also be modified during evaluation by filters and
    /// functions.
    ///
    /// Temps are useful for dynamically named data that needs to be represented
    /// as a [`Value`].  For ordinary typed Rust state, prefer
    /// [`get_or_insert_extension`](Self::get_or_insert_extension), which avoids
    /// object wrappers and interior mutability.
    ///
    /// # Example
    ///
    /// ```
    /// use minijinja::{Value, State};
    ///
    /// fn inc(state: &mut State) -> Value {
    ///     let old = state
    ///         .get_temp("my_counter")
    ///         .unwrap_or_else(|| Value::from(0i64));
    ///     let new = Value::from(i64::try_from(old).unwrap() + 1);
    ///     state.set_temp("my_counter", new.clone());
    ///     new
    /// }
    /// ```
    pub fn get_temp(&self, name: &str) -> Option<Value> {
        self.temps.get(name).cloned()
    }

    /// Inserts a temp and returns the old temp.
    ///
    /// For more information see [`get_temp`](Self::get_temp).
    pub fn set_temp(&mut self, name: &str, value: Value) -> Option<Value> {
        self.temps.insert(name.to_owned().into(), value)
    }

    /// Returns a reference to a typed render-local extension.
    ///
    /// Extensions are similar to [`temps`](Self::get_temp), but store ordinary
    /// Rust values keyed by their type.  They are useful for state that should
    /// be shared by filters and functions for the duration of a render without
    /// requiring a [`Value`], [`crate::value::Object`], or interior mutability.
    /// There can be one extension of each concrete type; use a newtype when independent
    /// values have the same underlying type.  Extension values must be `Send`
    /// because states can be moved between threads.
    ///
    /// Extensions are preserved across nested evaluation, including includes,
    /// blocks, and macro calls.  They are dropped together with the state.
    pub fn get_extension<T>(&self) -> Option<&T>
    where
        T: Send + 'static,
    {
        self.extensions
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref())
    }

    /// Returns a mutable reference to a typed render-local extension.
    ///
    /// For more information see [`get_extension`](Self::get_extension).
    pub fn get_extension_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Send + 'static,
    {
        self.extensions
            .get_mut(&TypeId::of::<T>())
            .and_then(|value| value.downcast_mut())
    }

    /// Returns a mutable extension, inserting it if necessary.
    ///
    /// If an extension of type `T` is already present, `value` is not inserted.
    /// Extensions require mutable state so that stored values do not need a
    /// mutex or other interior mutability.
    ///
    /// # Example
    ///
    /// ```
    /// use minijinja::State;
    ///
    /// #[derive(Default)]
    /// struct Counter(usize);
    ///
    /// fn next(state: &mut State) -> usize {
    ///     let counter = state.get_or_insert_extension(Counter::default());
    ///     counter.0 += 1;
    ///     counter.0
    /// }
    /// ```
    pub fn get_or_insert_extension<T>(&mut self, value: T) -> &mut T
    where
        T: Send + 'static,
    {
        self.get_or_insert_extension_with(|| value)
    }

    /// Returns a mutable extension, inserting one from `f` if necessary.
    ///
    /// For more information see [`get_or_insert_extension`](Self::get_or_insert_extension).
    pub fn get_or_insert_extension_with<T, F>(&mut self, f: F) -> &mut T
    where
        T: Send + 'static,
        F: FnOnce() -> T,
    {
        self.extensions
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(f()))
            .downcast_mut()
            .expect("extension had an unexpected type")
    }

    #[cfg(feature = "debug")]
    pub(crate) fn make_debug_info(
        &self,
        pc: u32,
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

/// Tracks a block and its parents for super.
#[derive(Clone, Default)]
pub(crate) struct BlockStack<'template, 'env> {
    instructions: Vec<&'template Instructions<'env>>,
    depth: usize,
}

impl<'template, 'env> BlockStack<'template, 'env> {
    pub fn new(instructions: &'template Instructions<'env>) -> BlockStack<'template, 'env> {
        BlockStack {
            instructions: vec![instructions],
            depth: 0,
        }
    }

    pub fn instructions(&self) -> &'template Instructions<'env> {
        self.instructions.get(self.depth).copied().unwrap()
    }

    #[cfg(feature = "multi_template")]
    pub fn len(&self) -> usize {
        self.instructions.len()
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
    pub fn append_instructions(&mut self, instructions: &'template Instructions<'env>) {
        self.instructions.push(instructions);
    }
}
