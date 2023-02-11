use std::collections::BTreeMap;
use std::{fmt, io};

use serde::Serialize;

use crate::compiler::codegen::CodeGenerator;
use crate::compiler::instructions::Instructions;
use crate::compiler::parser::parse;
use crate::environment::Environment;
use crate::error::{attach_basic_debug_info, Error, ErrorKind};
use crate::output::{Output, WriteWrapper};
use crate::utils::AutoEscape;
use crate::value::{self, Value};
use crate::vm::Vm;

/// Represents a handle to a template.
///
/// Templates are stored in the [`Environment`] as bytecode instructions.  With the
/// [`Environment::get_template`] method that is looked up and returned in form of
/// this handle.  Such a template can be cheaply copied as it only holds references.
///
/// To render the [`render`](Template::render) method can be used.
#[derive(Copy, Clone)]
pub struct Template<'env> {
    env: &'env Environment<'env>,
    compiled: &'env CompiledTemplate<'env>,
    initial_auto_escape: AutoEscape,
}

impl<'env> fmt::Debug for Template<'env> {
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

impl<'env> Template<'env> {
    pub(crate) fn new(
        env: &'env Environment<'env>,
        compiled: &'env CompiledTemplate<'env>,
        initial_auto_escape: AutoEscape,
    ) -> Template<'env> {
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
    pub fn render<S: Serialize>(&self, ctx: S) -> Result<String, Error> {
        // reduce total amount of code faling under mono morphization into
        // this function, and share the rest in _render.
        self.render_value(Value::from_serializable(&ctx))
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
    pub fn render_to_write<S: Serialize, W: io::Write>(&self, ctx: S, w: W) -> Result<(), Error> {
        let mut wrapper = WriteWrapper { w, err: None };
        self._eval(
            Value::from_serializable(&ctx),
            &mut Output::with_write(&mut wrapper),
        )
        .map(|_| ())
        .map_err(|err| {
            wrapper
                .err
                .take()
                .map(|io_err| {
                    Error::new(ErrorKind::WriteFailure, "I/O error during rendering")
                        .with_source(io_err)
                })
                .unwrap_or(err)
        })
    }

    /// Renders the template into a [`fmt::Write`].
    ///
    /// This works exactly like [`render`](Self::render) but instead writes the template
    /// as it's evaluating into a [`fmt::Write`].
    ///
    /// One reason to use this function instead of [`render`](Self::render) is to
    /// precisely allocate the render result's underlying [`String`] buffer:
    ///
    /// ```
    /// # use minijinja::{Environment, Output, context};
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "Hello {{ name }}!").unwrap();
    /// let tmpl = env.get_template("hello").unwrap();
    /// let mut buf = String::with_capacity(tmpl.size_hint() + 4);
    /// tmpl.render_to_fmt_write(context!(name => "Jane"), &mut buf).unwrap();
    /// println!("{}", buf);
    /// ```
    pub fn render_to_fmt_write<S: Serialize, W: fmt::Write>(
        &self,
        ctx: S,
        w: &mut W,
    ) -> Result<(), Error> {
        self._eval(Value::from_serializable(&ctx), &mut Output::with_write(w))
            .map(|_| ())
    }

    /// Renders the template into a string.
    ///
    /// This works exactly like [`render`](Self::render) but takes a monomorphized
    /// [`Value`] instead, allowing you to avoid reserializing if you are manipulating
    /// [`Value`] types directly.
    /// ```
    /// # use minijinja::{Environment, Output, context, value::Value};
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "Hello {{ name }}!").unwrap();
    /// let tmpl = env.get_template("hello").unwrap();
    /// let value: Value = [("name", "Anya")].into_iter().collect();
    /// tmpl.render_value(value).unwrap();
    /// ```
    pub fn render_value(&self, value: Value) -> Result<String, Error> {
        let mut rv = String::with_capacity(self.size_hint());
        self._eval(value, &mut Output::with_string(&mut rv))
            .map(|_| rv)
    }

    /// Renders the template into a [`fmt::Write`].
    ///
    /// This works exactly like [`render_to_fmt_write`](Self::render_to_fmt_write) but
    /// takes a monomorphized [`Value`] instead.
    /// ```
    /// # use minijinja::{Environment, Output, context, value::Value};
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "Hello {{ name }}!").unwrap();
    /// let tmpl = env.get_template("hello").unwrap();
    /// let value: Value = [("name", "Pavel")].into_iter().collect();
    /// let mut buf = String::with_capacity(tmpl.size_hint() + 5);
    /// tmpl.render_to_fmt_write(value, &mut buf).unwrap();
    /// println!("{}", buf);
    /// ```
    pub fn render_value_to_fmt_write<W: fmt::Write>(
        &self,
        value: Value,
        w: &mut W,
    ) -> Result<(), Error> {
        self._eval(value, &mut Output::with_write(w)).map(|_| ())
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

    /// A size hint for pre-allocating rendering buffers which uses the
    /// original template source length, in bytes. This is intentionally conservative
    /// and any sufficiently complex template logic or large context will cause the
    /// buffer to grow at least once during rendering.
    ///
    /// If the size of the context you are rendering is unknown you could consider
    /// hand-rolling your own heuristic, for example using [`usize::next_multiple_of`]
    /// or [`usize::next_power_of_two`]:
    ///
    /// ```
    /// # use minijinja::{Environment, context};
    /// # let mut env = Environment::new();
    /// # env.add_template("hello", "Hello {{ name }}!").unwrap();
    /// let tmpl = env.get_template("hello").unwrap();
    /// let size_hint = tmpl.size_hint().next_power_of_two();
    /// assert_eq!(tmpl.source().len(), 17);
    /// assert_eq!(size_hint, 32);
    /// ```
    pub fn size_hint(&self) -> usize {
        self.source().len()
    }

    /// Returns the root instructions.
    #[cfg(feature = "multi-template")]
    pub(crate) fn instructions(&self) -> &'env Instructions<'env> {
        &self.compiled.instructions
    }

    /// Returns the blocks.
    #[cfg(feature = "multi-template")]
    pub(crate) fn blocks(&self) -> &'env BTreeMap<&'env str, Instructions<'env>> {
        &self.compiled.blocks
    }

    /// Returns the initial auto escape setting.
    #[cfg(feature = "multi-template")]
    pub(crate) fn initial_auto_escape(&self) -> AutoEscape {
        self.initial_auto_escape
    }
}

/// Represents a compiled template in memory.
pub struct CompiledTemplate<'source> {
    /// The root instructions.
    pub instructions: Instructions<'source>,
    /// Block local instructions.
    pub blocks: BTreeMap<&'source str, Instructions<'source>>,
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
    /// Creates a compiled template from name and source.
    pub fn from_name_and_source(
        name: &'source str,
        source: &'source str,
    ) -> Result<CompiledTemplate<'source>, Error> {
        attach_basic_debug_info(Self::_from_name_and_source_impl(name, source), source)
    }

    fn _from_name_and_source_impl(
        name: &'source str,
        source: &'source str,
    ) -> Result<CompiledTemplate<'source>, Error> {
        // the parser/compiler combination can create constants in which case
        // we can probably benefit from the value optimization a bit.
        value::with_value_optimization(|| {
            let ast = ok!(parse(source, name));
            let mut gen = CodeGenerator::new(name, source);
            gen.compile_stmt(&ast);
            let (instructions, blocks) = gen.finish();
            Ok(CompiledTemplate {
                instructions,
                blocks,
            })
        })
    }
}
