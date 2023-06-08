use std::collections::{BTreeMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;
use std::{fmt, io};

use serde::Serialize;

use crate::compiler::codegen::CodeGenerator;
use crate::compiler::instructions::Instructions;
use crate::compiler::lexer::SyntaxConfig;
use crate::compiler::meta::find_undeclared;
use crate::compiler::parser::parse_with_syntax;
use crate::environment::Environment;
use crate::error::{attach_basic_debug_info, Error};
use crate::output::{Output, WriteWrapper};
use crate::utils::AutoEscape;
use crate::value::{self, Value};
use crate::vm::{prepare_blocks, State, Vm};

/// Represents a handle to a template.
///
/// Templates are stored in the [`Environment`] as bytecode instructions.  With the
/// [`Environment::get_template`] method that is looked up and returned in form of
/// this handle.  Such a template can be cheaply copied as it only holds references.
///
/// To render the [`render`](Template::render) method can be used.
#[derive(Clone)]
pub struct Template<'env: 'source, 'source> {
    env: &'env Environment<'env>,
    compiled: CompiledTemplateRef<'env, 'source>,
    initial_auto_escape: AutoEscape,
}

impl<'env, 'source> fmt::Debug for Template<'env, 'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("Template");
        ds.field("name", &self.name());
        #[cfg(feature = "internal_debug")]
        {
            ds.field("instructions", &self.compiled.instructions);
            ds.field("blocks", &self.compiled.blocks);
        }
        ds.field("initial_auto_escape", &self.initial_auto_escape);
        ds.finish()
    }
}

impl<'env, 'source> Template<'env, 'source> {
    pub(crate) fn new(
        env: &'env Environment<'env>,
        compiled: CompiledTemplateRef<'env, 'source>,
        initial_auto_escape: AutoEscape,
    ) -> Template<'env, 'source> {
        Template {
            env,
            compiled,
            initial_auto_escape,
        }
    }

    /// Returns the name of the template.
    pub fn name(&self) -> &str {
        self.compiled.instructions.name()
    }

    /// Returns the source code of the template.
    pub fn source(&self) -> &str {
        self.compiled.instructions.source()
    }

    /// Renders the template into a string.
    ///
    /// The provided value is used as the initial context for the template.  It
    /// can be any object that implements [`Serialize`](serde::Serialize).  You
    /// can either create your own struct and derive `Serialize` for it or the
    /// [`context!`](crate::context) macro can be used to create an ad-hoc context.
    ///
    /// ```
    /// # use minijinja::{Environment, context};
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "Hello {{ name }}!").unwrap();
    /// let tmpl = env.get_template("hello").unwrap();
    /// println!("{}", tmpl.render(context!(name => "John")).unwrap());
    /// ```
    ///
    /// **Note on values:** The [`Value`] type implements `Serialize` and can be
    /// efficiently passed to render.  It does not undergo actual serialization.
    pub fn render<S: Serialize>(&self, ctx: S) -> Result<String, Error> {
        // reduce total amount of code faling under mono morphization into
        // this function, and share the rest in _render.
        self._render(Value::from_serializable(&ctx))
    }

    fn _render(&self, root: Value) -> Result<String, Error> {
        let mut rv = String::with_capacity(self.compiled.buffer_size_hint);
        self._eval(root, &mut Output::with_string(&mut rv))
            .map(|_| rv)
    }

    /// Renders the template into a [`io::Write`].
    ///
    /// This works exactly like [`render`](Self::render) but instead writes the template
    /// as it's evaluating into a [`io::Write`].
    ///
    /// ```
    /// # use minijinja::{Environment, context};
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "Hello {{ name }}!").unwrap();
    /// use std::io::stdout;
    ///
    /// let tmpl = env.get_template("hello").unwrap();
    /// tmpl.render_to_write(context!(name => "John"), &mut stdout()).unwrap();
    /// ```
    ///
    /// **Note on values:** The [`Value`] type implements `Serialize` and can be
    /// efficiently passed to render.  It does not undergo actual serialization.
    pub fn render_to_write<S: Serialize, W: io::Write>(&self, ctx: S, w: W) -> Result<(), Error> {
        let mut wrapper = WriteWrapper { w, err: None };
        self._eval(
            Value::from_serializable(&ctx),
            &mut Output::with_write(&mut wrapper),
        )
        .map(|_| ())
        .map_err(|err| wrapper.take_err(err))
    }

    /// Evaluates the template into a [`Module`].
    ///
    /// This evaluates the template, discards the output and stores the final
    /// state in the evaluated module.  From there global variables or blocks
    /// can be accessed.  What this does is quite similar to how the engine
    /// interally works with tempaltes that are extended or imported from.
    ///
    /// For more information see [`Module`].
    pub fn eval_to_module<S: Serialize>(&self, ctx: S) -> Result<Module<'_, 'env>, Error> {
        let root = Value::from_serializable(&ctx);
        let mut out = Output::null();
        let vm = Vm::new(self.env);
        let state = ok!(vm.eval_to_module(
            &self.compiled.instructions,
            root,
            &self.compiled.blocks,
            &mut out,
            self.initial_auto_escape,
        ));
        Ok(Module { state })
    }

    fn _eval(&self, root: Value, out: &mut Output) -> Result<Option<Value>, Error> {
        Vm::new(self.env).eval(
            &self.compiled.instructions,
            root,
            &self.compiled.blocks,
            out,
            self.initial_auto_escape,
        )
    }

    /// Returns a set of all undeclared variables in the template.
    ///
    /// This returns a set of all variables that might be looked up
    /// at runtime by the template.  Since this is runs a static
    /// analysis, the actual control flow is not considered.  This
    /// also cannot take into account what happens due to includes,
    /// imports or extending.  If `nested` is set to `true`, then also
    /// nested trivial attribute lookups are considered and returned.
    ///
    /// ```rust
    /// # use minijinja::Environment;
    /// let mut env = Environment::new();
    /// env.add_template("x", "{% set x = foo %}{{ x }}{{ bar.baz }}").unwrap();
    /// let tmpl = env.get_template("x").unwrap();
    /// let undeclared = tmpl.undeclared_variables(false);
    /// // returns ["foo", "bar"]
    /// let undeclared = tmpl.undeclared_variables(true);
    /// // returns ["foo", "bar.baz"]
    /// ```
    pub fn undeclared_variables(&self, nested: bool) -> HashSet<String> {
        match parse_with_syntax(
            self.compiled.instructions.source(),
            self.name(),
            self.compiled.syntax.clone(),
        ) {
            Ok(ast) => find_undeclared(&ast, nested),
            Err(_) => HashSet::new(),
        }
    }

    /// Creates an empty [`State`] for this template.
    ///
    /// It's very rare that you need to actually do this but it can be useful when
    /// testing values or working with macros or other callable objects from outside
    /// the template environment.
    pub fn new_state(&self) -> State<'_, 'env> {
        State::new(
            self.env,
            Default::default(),
            self.initial_auto_escape,
            &self.compiled.instructions,
            prepare_blocks(&self.compiled.blocks),
        )
    }

    /// Returns the instructions and blocks if the template is loaded from the
    /// environment.
    ///
    /// For templates loaded as string on the environment this API contract
    /// cannot be upheld because the template might not live long enough.  Under
    /// normal cirumstances however such a template object would never make it
    /// to the callers of this API as this API is used for including or extending,
    /// both of which should only ever get access to a template from the environment
    /// which holds a borrowed ref.
    #[cfg(feature = "multi_template")]
    pub(crate) fn instructions_and_blocks(
        &self,
    ) -> Result<
        (
            &'env Instructions<'env>,
            &'env BTreeMap<&'env str, Instructions<'env>>,
        ),
        Error,
    > {
        match self.compiled {
            CompiledTemplateRef::Borrowed(x) => Ok((&x.instructions, &x.blocks)),
            CompiledTemplateRef::Owned(_) => Err(Error::new(
                crate::ErrorKind::InvalidOperation,
                "cannot extend or include template not borrowed from environment",
            )),
        }
    }

    /// Returns the initial auto escape setting.
    #[cfg(feature = "multi_template")]
    pub(crate) fn initial_auto_escape(&self) -> AutoEscape {
        self.initial_auto_escape
    }
}

/// Represents an evaluated template as module.
///
/// The engine internally sometimes treats templates as modules.  This happens
/// for instance for template inheritance or when importing global variables or
/// macros.  In the VM modules are virtual and never actually created, however
/// for some advanced use cases the same logic is exposed via this type.
///
/// This lets you call macros or blocks of a template from outside of the
/// template evaluation.  Because template modules hold mutable states, some
/// of the methods take `&mut self`.  The template module internally retains the
/// state of the execution and provides ways to extract some information from
/// it.
#[derive(Debug)]
pub struct Module<'template: 'env, 'env> {
    state: State<'template, 'env>,
}

impl<'template, 'env> Module<'template, 'env> {
    /// Returns the [`State`] of the module.
    pub fn state(&self) -> &State<'template, 'env> {
        &self.state
    }

    /// Renders a block with the given name into a string.
    ///
    /// This method works like [`render`](Template::render) but it only renders a specific
    /// block in the template.  The first argument is the name of the block.
    ///
    /// This renders only the block `hi` in the template:
    ///
    /// ```
    /// # use minijinja::{Environment, context};
    /// # fn test() -> Result<(), minijinja::Error> {
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "{% block hi %}Hello {{ name }}!{% endblock %}")?;
    /// let tmpl = env.get_template("hello")?;
    /// let rv = tmpl
    ///     .eval_to_module(context!(name => "John"))?
    ///     .render_block("hi")?;
    /// println!("{}", rv);
    /// # Ok(()) }
    /// ```
    ///
    /// Note that rendering a block is a stateful operation.  If an error
    /// is returned the module has to be re-created as the internal state
    /// can end up corrupted.
    #[cfg(feature = "multi_template")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multi_template")))]
    pub fn render_block(&mut self, block: &str) -> Result<String, Error> {
        let mut buf = String::new();
        Vm::new(self.state.env)
            .call_block(block, &mut self.state, &mut Output::with_string(&mut buf))
            .map(|_| buf)
    }

    /// Renders a block with the given name into a [`io::Write`].
    ///
    /// For details see [`render_block`](Self::render_block).
    #[cfg(feature = "multi_template")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multi_template")))]
    pub fn render_block_to_write<W>(&mut self, block: &str, w: W) -> Result<(), Error>
    where
        W: io::Write,
    {
        let mut wrapper = WriteWrapper { w, err: None };
        Vm::new(self.state.env)
            .call_block(
                block,
                &mut self.state,
                &mut Output::with_write(&mut wrapper),
            )
            .map(|_| ())
            .map_err(|err| wrapper.take_err(err))
    }

    /// Looks up a global macro and calls it.
    ///
    /// This looks up a value like [`get_export`](Self::get_export) does and
    /// calls it with the module's [`state`](Self::state) and the passed args.
    #[cfg(feature = "macros")]
    #[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
    pub fn call_macro(&self, name: &str, args: &[Value]) -> Result<String, Error> {
        let f = ok!(self.get_export(name).ok_or_else(|| Error::new(
            crate::error::ErrorKind::UnknownFunction,
            "macro not found"
        )));
        f.call(&self.state, args).map(Into::into)
    }

    /// Looks up a globally set variable in the template scope.
    pub fn get_export(&self, name: &str) -> Option<Value> {
        self.state.ctx.current_locals().get(name).cloned()
    }

    /// Returns a list of all exports.
    pub fn exports(&self) -> Vec<&str> {
        self.state.ctx.current_locals().keys().copied().collect()
    }
}

#[derive(Clone)]
pub(crate) enum CompiledTemplateRef<'env: 'source, 'source> {
    Owned(Arc<CompiledTemplate<'source>>),
    Borrowed(&'env CompiledTemplate<'source>),
}

impl<'env, 'source> Deref for CompiledTemplateRef<'env, 'source> {
    type Target = CompiledTemplate<'source>;

    fn deref(&self) -> &Self::Target {
        match self {
            CompiledTemplateRef::Owned(ref x) => x,
            CompiledTemplateRef::Borrowed(x) => x,
        }
    }
}

/// Represents a compiled template in memory.
pub struct CompiledTemplate<'source> {
    /// The root instructions.
    pub instructions: Instructions<'source>,
    /// Block local instructions.
    pub blocks: BTreeMap<&'source str, Instructions<'source>>,
    /// Optional size hint for string rendering.
    pub buffer_size_hint: usize,
    /// The syntax config that created it.
    pub syntax: SyntaxConfig,
}

impl<'env> fmt::Debug for CompiledTemplate<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("CompiledTemplate");
        #[cfg(feature = "internal_debug")]
        {
            ds.field("instructions", &self.instructions);
            ds.field("blocks", &self.blocks);
        }
        ds.finish()
    }
}

impl<'source> CompiledTemplate<'source> {
    /// Creates a compiled template from name and source using the given settings.
    pub fn new(
        name: &'source str,
        source: &'source str,
        syntax: SyntaxConfig,
    ) -> Result<CompiledTemplate<'source>, Error> {
        attach_basic_debug_info(Self::_new_impl(name, source, syntax), source)
    }

    fn _new_impl(
        name: &'source str,
        source: &'source str,
        syntax: SyntaxConfig,
    ) -> Result<CompiledTemplate<'source>, Error> {
        // the parser/compiler combination can create constants in which case
        // we can probably benefit from the value optimization a bit.
        let _guard = value::value_optimization();
        let ast = ok!(parse_with_syntax(source, name, syntax.clone()));
        let mut gen = CodeGenerator::new(name, source);
        gen.compile_stmt(&ast);
        let buffer_size_hint = gen.buffer_size_hint();
        let (instructions, blocks) = gen.finish();
        Ok(CompiledTemplate {
            instructions,
            blocks,
            buffer_size_hint,
            syntax,
        })
    }
}
