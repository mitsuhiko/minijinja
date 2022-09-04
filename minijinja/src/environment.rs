use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::Serialize;

use crate::compiler::Compiler;
use crate::error::Error;
use crate::instructions::Instructions;
use crate::parser::{parse, parse_expr};
use crate::utils::{AutoEscape, BTreeMapKeysDebug, HtmlEscape};
use crate::value::{ArgType, FunctionArgs, Value};
use crate::vm::Vm;
use crate::{filters, functions, tests};

#[cfg(test)]
use similar_asserts::assert_eq;

/// Represents a handle to a template.
///
/// Templates are stored in the [`Environment`] as bytecode instructions.  With the
/// [`Environment::get_template`] method that is looked up and returned in form of
/// this handle.  Such a template can be cheaply copied as it only holds two
/// pointers.  To render the [`render`](Template::render) method can be used.
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

/// Represents a compiled template in memory.
pub(crate) struct CompiledTemplate<'source> {
    instructions: Instructions<'source>,
    blocks: BTreeMap<&'source str, Instructions<'source>>,
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

fn attach_basic_debug_info<T>(rv: Result<T, Error>, source: &str) -> Result<T, Error> {
    #[cfg(feature = "debug")]
    {
        match rv {
            Ok(rv) => Ok(rv),
            Err(mut err) => {
                err.debug_info = Some(crate::error::DebugInfo {
                    template_source: Some(source.to_string()),
                    ..Default::default()
                });
                Err(err)
            }
        }
    }
    #[cfg(not(feature = "debug"))]
    {
        let _source = source;
        rv
    }
}

impl<'source> CompiledTemplate<'source> {
    pub(crate) fn from_name_and_source(
        name: &'source str,
        source: &'source str,
    ) -> Result<CompiledTemplate<'source>, Error> {
        attach_basic_debug_info(Self::_from_name_and_source_impl(name, source), source)
    }

    fn _from_name_and_source_impl(
        name: &'source str,
        source: &'source str,
    ) -> Result<CompiledTemplate<'source>, Error> {
        let ast = parse(source, name)?;
        let mut compiler = Compiler::new(name, source);
        compiler.compile_stmt(&ast)?;
        let (instructions, blocks) = compiler.finish();
        Ok(CompiledTemplate {
            blocks,
            instructions,
        })
    }
}

impl<'env> Template<'env> {
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
    /// can eiher create your own struct and derive `Serialize` for it or the
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
        // this function, and share the rest in _eval.
        self._render(Value::from_serializable(&ctx))
    }

    fn _render(&self, root: Value) -> Result<String, Error> {
        let mut output = String::new();
        let vm = Vm::new(self.env);
        let blocks = &self.compiled.blocks;
        vm.eval(
            &self.compiled.instructions,
            root,
            blocks,
            self.initial_auto_escape,
            &mut output,
        )?;
        Ok(output)
    }

    /// Returns the root instructions.
    pub(crate) fn instructions(&self) -> &'env Instructions<'env> {
        &self.compiled.instructions
    }

    /// Returns the blocks.
    pub(crate) fn blocks(&self) -> &'env BTreeMap<&'env str, Instructions<'env>> {
        &self.compiled.blocks
    }

    /// Returns the initial auto escape setting.
    pub(crate) fn initial_auto_escape(&self) -> AutoEscape {
        self.initial_auto_escape
    }
}

type TemplateMap<'source> = BTreeMap<&'source str, Arc<CompiledTemplate<'source>>>;

#[derive(Clone)]
enum Source<'source> {
    Borrowed(TemplateMap<'source>),
    #[cfg(feature = "source")]
    Owned(crate::source::Source),
}

impl<'source> fmt::Debug for Source<'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Borrowed(tmpls) => fmt::Debug::fmt(&BTreeMapKeysDebug(tmpls), f),
            #[cfg(feature = "source")]
            Self::Owned(arg0) => fmt::Debug::fmt(arg0, f),
        }
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
///
/// The environment holds references to the source the templates were created from.
/// This makes it very inconvenient to pass around unless the templates are static
/// strings.
#[cfg_attr(
    feature = "source",
    doc = "
For situations where you want to load dynamic templates and share the
environment it's recommended to turn on the `source` feature and to use the
[`Source`](crate::source::Source) type with the environment."
)]
#[derive(Clone)]
pub struct Environment<'source> {
    templates: Source<'source>,
    filters: BTreeMap<&'source str, filters::BoxedFilter>,
    tests: BTreeMap<&'source str, tests::BoxedTest>,
    pub(crate) globals: BTreeMap<&'source str, Value>,
    default_auto_escape: Arc<dyn Fn(&str) -> AutoEscape + Sync + Send>,
    #[cfg(feature = "debug")]
    debug: bool,
}

impl<'source> Default for Environment<'source> {
    fn default() -> Self {
        Environment::empty()
    }
}

impl<'source> fmt::Debug for Environment<'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Environment")
            .field("globals", &self.globals)
            .field("tests", &BTreeMapKeysDebug(&self.tests))
            .field("filters", &BTreeMapKeysDebug(&self.filters))
            .field("templates", &self.templates)
            .finish()
    }
}

fn default_auto_escape(name: &str) -> AutoEscape {
    match name.rsplit('.').next() {
        Some("html") | Some("htm") | Some("xml") => AutoEscape::Html,
        #[cfg(feature = "json")]
        Some("json") | Some("js") | Some("yaml") | Some("yml") => AutoEscape::Json,
        _ => AutoEscape::None,
    }
}

fn no_auto_escape(_: &str) -> AutoEscape {
    AutoEscape::None
}

/// A handle to a compiled expression.
///
/// An expression is created via the
/// [`compile_expression`](Environment::compile_expression) method.  It provides
/// a method to evaluate the expression and return the result as value object.
/// This for instance can be used to evaluate simple expressions from user
/// provided input to implement features such as dynamic filtering.
///
/// This is usually best paired with [`context`](crate::context!) to pass
/// a single value to it.
///
/// # Example
///
/// ```rust
/// # use minijinja::{Environment, context};
/// let env = Environment::new();
/// let expr = env.compile_expression("number > 10 and number < 20").unwrap();
/// let rv = expr.eval(context!(number => 15)).unwrap();
/// assert!(rv.is_true());
/// ```
pub struct Expression<'env, 'source> {
    env: &'env Environment<'source>,
    instructions: Instructions<'source>,
}

impl<'env, 'source> fmt::Debug for Expression<'env, 'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Expression")
            .field("env", &self.env)
            .finish()
    }
}

impl<'env, 'source> Expression<'env, 'source> {
    /// Evaluates the expression with some context.
    ///
    /// The result of the expression is returned as [`Value`].
    pub fn eval<S: Serialize>(&self, ctx: S) -> Result<Value, Error> {
        // reduce total amount of code faling under mono morphization into
        // this function, and share the rest in _eval.
        self._eval(Value::from_serializable(&ctx))
    }

    fn _eval(&self, root: Value) -> Result<Value, Error> {
        let mut output = String::new();
        let vm = Vm::new(self.env);
        let blocks = BTreeMap::new();
        Ok(vm
            .eval(
                &self.instructions,
                root,
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
    /// This environment does not yet contain any templates but it will have all
    /// the default filters, tests and globals loaded.  If you do not want any
    /// default configuration you can use the alternative
    /// [`empty`](Environment::empty) method.
    pub fn new() -> Environment<'source> {
        Environment {
            templates: Source::Borrowed(Default::default()),
            filters: filters::get_builtin_filters(),
            tests: tests::get_builtin_tests(),
            globals: functions::get_globals(),
            default_auto_escape: Arc::new(default_auto_escape),
            #[cfg(feature = "debug")]
            debug: cfg!(debug_assertions),
        }
    }

    /// Creates a completely empty environment.
    ///
    /// This environment has no filters, no templates, no globals and no default
    /// logic for auto escaping configured.
    pub fn empty() -> Environment<'source> {
        Environment {
            templates: Source::Borrowed(Default::default()),
            filters: Default::default(),
            tests: Default::default(),
            globals: Default::default(),
            default_auto_escape: Arc::new(no_auto_escape),
            #[cfg(feature = "debug")]
            debug: cfg!(debug_assertions),
        }
    }

    /// Sets a new function to select the default auto escaping.
    ///
    /// This function is invoked when templates are loaded from the environment
    /// to determine the default auto escaping behavior.  The function is
    /// invoked with the name of the template and can make an initial auto
    /// escaping decision based on that.  The default implementation is to
    /// turn on escaping depending on the file extension:
    ///
    /// * [`Html`](AutoEscape::Html): `.html`, `.htm`, `.xml`
    #[cfg_attr(
        feature = "json",
        doc = r" * [`Json`](AutoEscape::Json): `.json`, `.js`, `.yml`"
    )]
    /// * [`None`](AutoEscape::None): _all others_
    pub fn set_auto_escape_callback<F: Fn(&str) -> AutoEscape + 'static + Sync + Send>(
        &mut self,
        f: F,
    ) {
        self.default_auto_escape = Arc::new(f);
    }

    /// Enable or disable the debug mode.
    ///
    /// When the debug mode is enabled the engine will dump out some of the
    /// execution state together with the source information of the executing
    /// template when an error is created.  The cost of this is relatively
    /// high as the data including the template source is cloned.
    ///
    /// However providing this information greatly improves the debug information
    /// that the template error provides.  When debug is enabled errors will
    /// return a [`DebugInfo`](crate::error::DebugInfo) object from
    /// [`Error::debug_info`](crate::error::Error::debug_info).
    ///
    /// This requires the `debug` feature.  This is enabled by default if
    /// debug assertions are enabled and false otherwise.
    #[cfg(feature = "debug")]
    #[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug = enabled;
    }

    #[cfg(feature = "debug")]
    pub(crate) fn debug(&self) -> bool {
        self.debug
    }

    /// Sets the template source for the environment.
    ///
    /// This helps when working with dynamically loaded templates.  The
    /// [`Source`](crate::source::Source) is consulted by the environment to
    /// look up templates that are requested.  The source has the capabilities
    /// to load templates with fewer lifetime restrictions and can also
    /// load templates dynamically at runtime as requested.
    ///
    /// When a source is set already loaded templates in the environment are
    /// discarded and replaced with the templates from the source.
    ///
    /// For more information see [`Source`](crate::source::Source).
    #[cfg(feature = "source")]
    #[cfg_attr(docsrs, doc(cfg(feature = "source")))]
    pub fn set_source(&mut self, source: crate::source::Source) {
        self.templates = Source::Owned(source);
    }

    /// Returns the currently set source.
    #[cfg(feature = "source")]
    #[cfg_attr(docsrs, doc(cfg(feature = "source")))]
    pub fn source(&self) -> Option<&crate::source::Source> {
        match self.templates {
            Source::Borrowed(_) => None,
            Source::Owned(ref source) => Some(source),
        }
    }

    /// Loads a template from a string.
    ///
    /// The `name` parameter defines the name of the template which identifies
    /// it.  To look up a loaded template use the [`get_template`](Self::get_template)
    /// method.
    ///
    /// Note that there are situations where the interface of this method is
    /// too restrictive.  For instance the environment itself does not permit
    /// any form of sensible dynamic template loading.
    #[cfg_attr(
        feature = "source",
        doc = "To address this restriction use [`set_source`](Self::set_source)."
    )]
    pub fn add_template(&mut self, name: &'source str, source: &'source str) -> Result<(), Error> {
        match self.templates {
            Source::Borrowed(ref mut map) => {
                let compiled_template = CompiledTemplate::from_name_and_source(name, source)?;
                map.insert(name, Arc::new(compiled_template));
                Ok(())
            }
            #[cfg(feature = "source")]
            Source::Owned(ref mut src) => src.add_template(name, source),
        }
    }

    /// Removes a template by name.
    pub fn remove_template(&mut self, name: &str) {
        match self.templates {
            Source::Borrowed(ref mut map) => {
                map.remove(name);
            }
            #[cfg(feature = "source")]
            Source::Owned(ref mut source) => {
                source.remove_template(name);
            }
        }
    }

    /// Fetches a template by name.
    ///
    /// This requires that the template has been loaded with
    /// [`add_template`](Environment::add_template) beforehand.  If the template was
    /// not loaded an error of kind `TemplateNotFound` is returned.
    ///
    /// ```
    /// # use minijinja::{Environment, context};
    /// let mut env = Environment::new();
    /// env.add_template("hello.txt", "Hello {{ name }}!").unwrap();
    /// let tmpl = env.get_template("hello.txt").unwrap();
    /// println!("{}", tmpl.render(context!{ name => "World" }).unwrap());
    /// ```
    pub fn get_template(&self, name: &str) -> Result<Template<'_>, Error> {
        let compiled = match &self.templates {
            Source::Borrowed(ref map) => map.get(name).ok_or_else(|| Error::new_not_found(name))?,
            #[cfg(feature = "source")]
            Source::Owned(source) => source.get_compiled_template(name)?,
        };
        Ok(Template {
            env: self,
            compiled,
            initial_auto_escape: self.get_initial_auto_escape(name),
        })
    }

    /// Parses and renders a template from a string in one go.
    ///
    /// In some cases you really only need a template to be rendered once from
    /// a string and returned.  The internal name of the template is `<string>`.
    ///
    /// ```
    /// # use minijinja::{Environment, context};
    /// let env = Environment::new();
    /// let rv = env.render_str("Hello {{ name }}", context! { name => "World" });
    /// println!("{}", rv.unwrap());
    /// ```
    pub fn render_str<S: Serialize>(&self, source: &str, ctx: S) -> Result<String, Error> {
        // reduce total amount of code faling under mono morphization into
        // this function, and share the rest in _eval.
        self._render_str(source, Value::from_serializable(&ctx))
    }

    fn _render_str(&self, source: &str, root: Value) -> Result<String, Error> {
        let name = "<string>";
        let compiled = CompiledTemplate::from_name_and_source(name, source)?;
        let mut output = String::new();
        let vm = Vm::new(self);
        let blocks = &compiled.blocks;
        let initial_auto_escape = self.get_initial_auto_escape(name);
        vm.eval(
            &compiled.instructions,
            root,
            blocks,
            initial_auto_escape,
            &mut output,
        )?;
        Ok(output)
    }

    /// Compiles an expression.
    ///
    /// This lets one compile an expression in the template language and
    /// receive the output.  This lets one use the expressions of the language
    /// be used as a minimal scripting language.  For more information and an
    /// example see [`Expression`].
    pub fn compile_expression(&self, expr: &'source str) -> Result<Expression<'_, 'source>, Error> {
        attach_basic_debug_info(self._compile_expression(expr), expr)
    }

    fn _compile_expression(&self, expr: &'source str) -> Result<Expression<'_, 'source>, Error> {
        let ast = parse_expr(expr)?;
        let mut compiler = Compiler::new("<expression>", expr);
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
        V: for<'a> ArgType<'a>,
        Rv: Into<Value>,
        F: filters::Filter<V, Rv, Args>,
        Args: for<'a> FunctionArgs<'a>,
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
    pub fn add_test<F, V, Args>(&mut self, name: &'source str, f: F)
    where
        V: for<'a> ArgType<'a>,
        F: tests::Test<V, Args>,
        Args: for<'a> FunctionArgs<'a>,
    {
        self.tests.insert(name, tests::BoxedTest::new(f));
    }

    /// Removes a test by name.
    pub fn remove_test(&mut self, name: &str) {
        self.tests.remove(name);
    }

    /// Adds a new global function.
    ///
    /// For details about functions have a look at [`functions`].  Note that
    /// functions and other global variables share the same namespace.
    pub fn add_function<F, Rv, Args>(&mut self, name: &'source str, f: F)
    where
        Rv: Into<Value>,
        F: functions::Function<Rv, Args>,
        Args: for<'a> FunctionArgs<'a>,
    {
        self.add_global(name, functions::BoxedFunction::new(f).to_value());
    }

    /// Adds a global variable.
    pub fn add_global(&mut self, name: &'source str, value: Value) {
        self.globals.insert(name, value);
    }

    /// Removes a global function or variable by name.
    pub fn remove_global(&mut self, name: &str) {
        self.globals.remove(name);
    }

    /// Looks up a function.
    pub(crate) fn get_global(&self, name: &str) -> Option<Value> {
        self.globals.get(name).cloned()
    }

    /// Looks up a filter.
    pub(crate) fn get_filter(&self, name: &str) -> Option<&filters::BoxedFilter> {
        self.filters.get(name)
    }

    /// Looks up a test function.
    pub(crate) fn get_test(&self, name: &str) -> Option<&tests::BoxedTest> {
        self.tests.get(name)
    }

    pub(crate) fn get_initial_auto_escape(&self, name: &str) -> AutoEscape {
        (self.default_auto_escape)(name)
    }

    pub(crate) fn escape(
        &self,
        value: &Value,
        autoescape: AutoEscape,
        out: &mut String,
    ) -> Result<(), Error> {
        use std::fmt::Write;

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
            #[cfg(feature = "json")]
            AutoEscape::Json => {
                let value = serde_json::to_string(&value).map_err(|err| {
                    Error::new(
                        crate::ErrorKind::BadSerialization,
                        "unable to format to JSON",
                    )
                    .with_source(err)
                })?;
                write!(out, "{}", value).unwrap()
            }
        }
        Ok(())
    }

    /// Finalizes a value.
    pub(crate) fn finalize(
        &self,
        value: &Value,
        autoescape: AutoEscape,
        out: &mut String,
    ) -> Result<(), Error> {
        self.escape(value, autoescape, out)
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

#[test]
fn test_clone() {
    let mut env = Environment::new();
    env.add_template("test", "a").unwrap();
    let mut env2 = env.clone();
    assert_eq!(env2.get_template("test").unwrap().render(&()).unwrap(), "a");
    env2.add_template("test", "b").unwrap();
    assert_eq!(env2.get_template("test").unwrap().render(&()).unwrap(), "b");
    assert_eq!(env.get_template("test").unwrap().render(&()).unwrap(), "a");
}

#[test]
fn test_globals() {
    let mut env = Environment::new();
    env.add_global("a", Value::from(42));
    env.add_template("test", "{{ a }}").unwrap();
    let tmpl = env.get_template("test").unwrap();
    assert_eq!(tmpl.render(()).unwrap(), "42");
}

#[test]
fn test_template_removal() {
    let mut env = Environment::new();
    env.add_template("test", "{{ a }}").unwrap();
    env.remove_template("test");
    assert!(env.get_template("test").is_err());
}
