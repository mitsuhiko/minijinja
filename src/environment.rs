use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;

use serde::Serialize;

use crate::compiler::Compiler;
use crate::error::{Error, ErrorKind};
use crate::instructions::Instructions;
use crate::parser::{parse, parse_expr};
use crate::utils::{AutoEscape, HtmlEscape};
use crate::value::{Value, ValueArgs};
use crate::vm::Vm;
use crate::{filters, tests};

/// Represents a handle to a template.
///
/// Templates are stored in the [`Environment`] as bytecode instructions.  With the
/// [`Environment::get_template`] method that is looked up and returned in form of
/// this handle.  Such a template can be cheaply copied as it only holds two
/// pointers.  To render the [`render`](Template::render) method can be used.
#[derive(Copy, Clone)]
pub struct Template<'env, 'source> {
    env: &'env Environment<'env>,
    compiled: &'env CompiledTemplate<'source>,
}

impl<'env, 'source> fmt::Debug for Template<'env, 'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Template")
            .field("name", &self.compiled.name)
            .field("instructions", &self.compiled.instructions)
            .field("blocks", &self.compiled.blocks)
            .field("initial_auto_escape", &self.compiled.initial_auto_escape)
            .finish()
    }
}

/// Represents a compiled template in memory.
#[derive(Debug)]
pub struct CompiledTemplate<'source> {
    name: &'source str,
    instructions: Instructions<'source>,
    blocks: BTreeMap<&'source str, Instructions<'source>>,
    initial_auto_escape: AutoEscape,
}

impl<'env, 'source> Template<'env, 'source> {
    /// Returns the name of the template.
    pub fn name(&self) -> &str {
        self.compiled.name
    }

    /// Renders the template into a string.
    ///
    /// The provided value is used as the initial context for the template.  It
    /// can be any object that implements [`Serialize`](serde::Serialize).
    /// Typically custom structs annotated with `#[derive(Serialize)]` would
    /// be used for this purpose.
    pub fn render<S: Serialize>(&self, ctx: S) -> Result<String, Error> {
        let mut output = String::new();
        let vm = Vm::new(self.env);
        let blocks = &self.compiled.blocks;
        vm.eval(
            &self.compiled.instructions,
            ctx,
            blocks,
            self.compiled.initial_auto_escape,
            &mut output,
        )?;
        Ok(output)
    }

    /// Returns the root instructions.
    pub(crate) fn instructions(&self) -> &'env Instructions<'source> {
        &self.compiled.instructions
    }

    /// Returns the blocks.
    pub(crate) fn blocks(&self) -> &'env BTreeMap<&'source str, Instructions<'source>> {
        &self.compiled.blocks
    }
}

/// An abstraction that holds the engine configuration.
///
/// This object holds the central configuration state for templates and their
/// configuration.  Instances of this type may be modified if no template were
/// loaded so far.  Modifications on environments after the first template was
/// loaded will lead to surprising effects and undefined behavior.  For instance
/// overriding the auto escape callback will no longer have effects to an already
/// loaded template.
pub struct Environment<'source> {
    templates: BTreeMap<&'source str, CompiledTemplate<'source>>,
    filters: BTreeMap<&'source str, filters::BoxedFilter>,
    tests: BTreeMap<&'source str, tests::BoxedTest>,
    default_auto_escape: Box<dyn Fn(&str) -> AutoEscape>,
}

impl<'source> Default for Environment<'source> {
    fn default() -> Self {
        Environment::empty()
    }
}

impl<'source> fmt::Debug for Environment<'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Environment")
            .field("templates", &self.templates)
            .finish()
    }
}

fn default_auto_escape(name: &str) -> AutoEscape {
    match name.rsplit('.').next() {
        Some("html" | "htm" | "xml") => AutoEscape::Html,
        _ => AutoEscape::None,
    }
}

fn no_auto_escape(_: &str) -> AutoEscape {
    AutoEscape::None
}

/// A handle to a compiled expression.
#[derive(Debug)]
pub struct Expression<'env, 'source> {
    env: &'env Environment<'source>,
    instructions: Instructions<'source>,
}

impl<'env, 'source> Expression<'env, 'source> {
    pub fn eval<S: Serialize>(&self, ctx: S) -> Result<Value, Error> {
        let mut output = String::new();
        let vm = Vm::new(self.env);
        let blocks = BTreeMap::new();
        Ok(vm
            .eval(
                &self.instructions,
                ctx,
                &blocks,
                AutoEscape::None,
                &mut output,
            )?
            .unwrap())
    }
}

impl<'source> Environment<'source> {
    /// Creates a new environment with sensible defaults.
    ///
    /// This environment does not yet contain any templates but it will have all the
    /// default filters loaded.  If you do not want any default configuration you
    /// can use the alternative [`empty`](Environment::empty) method.
    pub fn new() -> Environment<'source> {
        Environment {
            templates: BTreeMap::new(),
            filters: filters::get_default_filters(),
            tests: tests::get_default_tests(),
            default_auto_escape: Box::new(default_auto_escape),
        }
    }

    /// Creates a completely empty environment.
    ///
    /// This environment has no filters, no templates and no default logic for
    /// auto escaping configured.
    pub fn empty() -> Environment<'source> {
        Environment {
            templates: BTreeMap::new(),
            filters: BTreeMap::new(),
            tests: BTreeMap::new(),
            default_auto_escape: Box::new(no_auto_escape),
        }
    }

    /// Sets a new function to select the default auto escaping.
    ///
    /// This function is invoked when templates are added to the environment
    /// to determine the default auto escaping behavior.  The function is
    /// invoked with the name of the template and can make an initial auto
    /// escaping decision based on that.  The default implementation is to
    /// turn on escaping for templates ending with `.html`, `.htm` and `.xml`.
    pub fn set_auto_escape_callback<F: Fn(&str) -> AutoEscape + 'static>(&mut self, f: F) {
        self.default_auto_escape = Box::new(f);
    }

    /// Loads a template from a string.
    ///
    /// The `name` parameter defines the name of the template which identifies
    /// it.  To look up a loaded template use the [`get_template`](Self::get_template)
    /// method.
    pub fn add_template(&mut self, name: &'source str, source: &'source str) -> Result<(), Error> {
        let ast = parse(source, name)?;
        let mut compiler = Compiler::new();
        compiler.compile_stmt(&ast)?;
        let (instructions, blocks) = compiler.finish();
        self.templates.insert(
            name,
            CompiledTemplate {
                name,
                blocks,
                instructions,
                initial_auto_escape: (self.default_auto_escape)(name),
            },
        );
        Ok(())
    }

    /// Removes a template by name.
    pub fn remove_template(&mut self, name: &str) {
        self.templates.remove(name);
    }

    /// Fetches a template by name.
    ///
    /// This requires that the template has been loaded with
    /// [`add_template`](Environment::add_template) beforehand.  If the template was
    /// not loaded `None` is returned.
    pub fn get_template(&self, name: &str) -> Option<Template<'_, 'source>> {
        self.templates.get(name).map(|compiled| Template {
            env: self,
            compiled,
        })
    }

    /// Compiles an expression.
    ///
    /// This lets one compile an expression in the template language and
    /// receive the output.  This lets one use the expressions of the language
    /// be used as a minimal scripting language.
    pub fn compile_expression(&self, expr: &'source str) -> Result<Expression<'_, 'source>, Error> {
        let ast = parse_expr(expr)?;
        let mut compiler = Compiler::new();
        compiler.compile_expr(&ast)?;
        let (instructions, _) = compiler.finish();
        Ok(Expression {
            env: self,
            instructions,
        })
    }

    /// Adds a new filter function.
    ///
    /// For details about filters have a look at [`filters`].
    pub fn add_filter<F, V, Rv, Args>(&mut self, name: &'source str, f: F)
    where
        V: TryFrom<Value>,
        Rv: Into<Value>,
        F: filters::Filter<V, Rv, Args>,
        Args: ValueArgs,
    {
        self.filters.insert(name, filters::BoxedFilter::new(f));
    }

    /// Removes a filter by name.
    pub fn remove_filter(&mut self, name: &str) {
        self.filters.remove(name);
    }

    /// Adds a new test function.
    ///
    /// For details about tests have a look at [`tests`].
    pub fn add_test<F, V, Rv, Args>(&mut self, name: &'source str, f: F)
    where
        V: TryFrom<Value>,
        F: tests::Test<V, Args>,
        Args: ValueArgs,
    {
        self.tests.insert(name, tests::BoxedTest::new(f));
    }

    /// Removes a test by name.
    pub fn remove_test(&mut self, name: &str) {
        self.tests.remove(name);
    }

    /// Applies a filter with arguments to a value.
    pub(crate) fn apply_filter(
        &self,
        name: &str,
        value: Value,
        args: Vec<Value>,
    ) -> Result<Value, Error> {
        if let Some(filter) = self.filters.get(name) {
            filter.apply_to(self, value, args)
        } else {
            Err(Error::new(
                ErrorKind::UnknownFilter,
                format!("filter {} is unknown", name),
            ))
        }
    }

    /// Performs a test.
    pub(crate) fn perform_test(
        &self,
        name: &str,
        value: Value,
        args: Vec<Value>,
    ) -> Result<bool, Error> {
        if let Some(test) = self.tests.get(name) {
            test.perform(self, value, args)
        } else {
            Err(Error::new(
                ErrorKind::UnknownTest,
                format!("test {} is unknown", name),
            ))
        }
    }

    /// Finalizes a value.
    pub(crate) fn finalize<W: fmt::Write>(
        &self,
        value: &Value,
        autoescape: AutoEscape,
        out: &mut W,
    ) -> Result<(), Error> {
        // safe values do not get escaped
        if value.is_safe() {
            write!(out, "{}", value).unwrap();
            return Ok(());
        }

        // TODO: this should become pluggable
        match autoescape {
            AutoEscape::None => write!(out, "{}", value).unwrap(),
            AutoEscape::Html => {
                if let Some(s) = value.as_str() {
                    write!(out, "{}", HtmlEscape(s)).unwrap()
                } else {
                    write!(out, "{}", HtmlEscape(&value.to_string())).unwrap()
                }
            }
        }
        Ok(())
    }
}

#[test]
fn test_basic() {
    use crate::value::Value;

    let mut env = Environment::new();
    env.add_template("test", "{% for x in seq %}[{{ x }}]{% endfor %}")
        .unwrap();
    let t = env.get_template("test").unwrap();
    let mut ctx = BTreeMap::new();
    ctx.insert("seq", Value::from((0..3).collect::<Vec<_>>()));
    let rv = t.render(ctx).unwrap();
    assert_eq!(rv, "[0][1][2]");
}

#[test]
fn test_expression() {
    let env = Environment::new();
    let expr = env.compile_expression("foo + bar").unwrap();
    let mut ctx = BTreeMap::new();
    ctx.insert("foo", 42);
    ctx.insert("bar", 23);
    assert_eq!(expr.eval(&ctx).unwrap(), Value::from(65));
}

#[test]
fn test_expression_lifetimes() {
    let mut env = Environment::new();
    let s = String::new();
    env.add_template("test", &s).unwrap();
    {
        let x = String::from("1 + 1");
        let expr = env.compile_expression(&x).unwrap();
        assert_eq!(expr.eval(&()).unwrap().to_string(), "2");
    }
}
