use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

use crate::compiler::Compiler;
use crate::error::{Error, ErrorKind};
use crate::instructions::Instructions;
use crate::parser::{parse, parse_expr};
use crate::utils::{AutoEscape, BTreeMapKeysDebug, HtmlEscape};
use crate::value::{ArgType, FunctionArgs, RcType, Value};
use crate::vm::Vm;
use crate::{filters, functions, tests};

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
    name: &'env str,
    initial_auto_escape: AutoEscape,
}

impl<'env> fmt::Debug for Template<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Template")
            .field("name", &self.name)
            .field("instructions", &self.compiled.instructions)
            .field("blocks", &self.compiled.blocks)
            .field("initial_auto_escape", &self.initial_auto_escape)
            .finish()
    }
}

/// Represents a compiled template in memory.
#[derive(Clone)]
pub(crate) struct CompiledTemplate<'source> {
    instructions: Instructions<'source>,
    blocks: BTreeMap<&'source str, Instructions<'source>>,
}

impl<'env> fmt::Debug for CompiledTemplate<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledTemplate")
            .field("instructions", &self.instructions)
            .field("blocks", &self.blocks)
            .finish()
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
        self.name
    }

    /// Returns the source code of the template.
    pub fn source(&self) -> &str {
        self.compiled.instructions.source()
    }

    /// Renders the template into a string.
    ///
    /// The provided value is used as the initial context for the template.  It
    /// can be any object that implements [`Serialize`](serde::Serialize).
    /// Typically custom structs annotated with `#[derive(Serialize)]` would
    /// be used for this purpose.
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

type TemplateMap<'source> = BTreeMap<&'source str, RcType<CompiledTemplate<'source>>>;

#[derive(Clone)]
enum Source<'source> {
    Borrowed(RcType<TemplateMap<'source>>),
    #[cfg(feature = "source")]
    Owned(RcType<crate::source::Source>),
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
    filters: RcType<BTreeMap<&'source str, filters::BoxedFilter>>,
    tests: RcType<BTreeMap<&'source str, tests::BoxedTest>>,
    pub(crate) globals: RcType<BTreeMap<&'source str, Value>>,
    default_auto_escape: RcType<dyn Fn(&str) -> AutoEscape + Sync + Send>,
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
#[derive(Debug)]
pub struct Expression<'env, 'source> {
    env: &'env Environment<'source>,
    instructions: Instructions<'source>,
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
    /// This environment does not yet contain any templates but it will have all the
    /// default filters loaded.  If you do not want any default configuration you
    /// can use the alternative [`empty`](Environment::empty) method.
    pub fn new() -> Environment<'source> {
        Environment {
            templates: Source::Borrowed(Default::default()),
            filters: RcType::new(filters::get_builtin_filters()),
            tests: RcType::new(tests::get_builtin_tests()),
            globals: RcType::new(functions::get_globals()),
            default_auto_escape: RcType::new(default_auto_escape),
            #[cfg(feature = "debug")]
            debug: false,
        }
    }

    /// Creates a completely empty environment.
    ///
    /// This environment has no filters, no templates and no default logic for
    /// auto escaping configured.
    pub fn empty() -> Environment<'source> {
        Environment {
            templates: Source::Borrowed(Default::default()),
            filters: RcType::default(),
            tests: RcType::default(),
            globals: RcType::default(),
            default_auto_escape: RcType::new(no_auto_escape),
            #[cfg(feature = "debug")]
            debug: false,
        }
    }

    /// Sets a new function to select the default auto escaping.
    ///
    /// This function is invoked when templates are loaded from the environment
    /// to determine the default auto escaping behavior.  The function is
    /// invoked with the name of the template and can make an initial auto
    /// escaping decision based on that.  The default implementation is to
    /// turn on escaping for templates ending with `.html`, `.htm` and `.xml`.
    pub fn set_auto_escape_callback<F: Fn(&str) -> AutoEscape + 'static + Sync + Send>(
        &mut self,
        f: F,
    ) {
        self.default_auto_escape = RcType::new(f);
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
    /// This requires the `debug` feature.
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
    /// This helps when working with dynamically loaded templates.  For more
    /// information see [`Source`](crate::source::Source).
    ///
    /// Already loaded templates in the environment are discarded and replaced
    /// with the templates from the source.
    #[cfg(feature = "source")]
    #[cfg_attr(docsrs, doc(cfg(feature = "source")))]
    pub fn set_source(&mut self, source: crate::source::Source) {
        self.templates = Source::Owned(RcType::new(source));
    }

    /// Returns the currently set source.
    #[cfg(feature = "source")]
    #[cfg_attr(docsrs, doc(cfg(feature = "source")))]
    pub fn get_source(&self) -> Option<&crate::source::Source> {
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
    pub fn add_template(&mut self, name: &'source str, source: &'source str) -> Result<(), Error> {
        match self.templates {
            Source::Borrowed(ref mut map) => {
                let compiled_template = CompiledTemplate::from_name_and_source(name, source)?;
                RcType::make_mut(map).insert(name, RcType::new(compiled_template));
                Ok(())
            }
            #[cfg(feature = "source")]
            Source::Owned(ref mut src) => RcType::make_mut(src).add_template(name, source),
        }
    }

    /// Removes a template by name.
    pub fn remove_template(&mut self, name: &str) {
        match self.templates {
            Source::Borrowed(ref mut map) => {
                RcType::make_mut(map).remove(name);
            }
            #[cfg(feature = "source")]
            Source::Owned(ref mut source) => {
                RcType::make_mut(source).remove_template(name);
            }
        }
    }

    /// Fetches a template by name.
    ///
    /// This requires that the template has been loaded with
    /// [`add_template`](Environment::add_template) beforehand.  If the template was
    /// not loaded an error of kind `TemplateNotFound` is returned.
    pub fn get_template(&self, name: &str) -> Result<Template<'_>, Error> {
        let rv = match &self.templates {
            Source::Borrowed(ref map) => map.get_key_value(name).map(|(&k, v)| (k, &**v)),
            #[cfg(feature = "source")]
            Source::Owned(source) => source.get_compiled_template(name),
        };
        rv.map(|(name, compiled)| Template {
            env: self,
            compiled,
            name,
            initial_auto_escape: (self.default_auto_escape)(name),
        })
        .ok_or_else(|| {
            Error::new(
                ErrorKind::TemplateNotFound,
                format!("template name {:?}", name),
            )
        })
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

    pub fn _compile_expression(
        &self,
        expr: &'source str,
    ) -> Result<Expression<'_, 'source>, Error> {
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
        V: ArgType,
        Rv: Into<Value>,
        F: filters::Filter<V, Rv, Args>,
        Args: FunctionArgs,
    {
        RcType::make_mut(&mut self.filters).insert(name, filters::BoxedFilter::new(f));
    }

    /// Removes a filter by name.
    pub fn remove_filter(&mut self, name: &str) {
        RcType::make_mut(&mut self.filters).remove(name);
    }

    /// Adds a new test function.
    ///
    /// For details about tests have a look at [`tests`].
    pub fn add_test<F, V, Args>(&mut self, name: &'source str, f: F)
    where
        V: ArgType,
        F: tests::Test<V, Args>,
        Args: FunctionArgs,
    {
        RcType::make_mut(&mut self.tests).insert(name, tests::BoxedTest::new(f));
    }

    /// Removes a test by name.
    pub fn remove_test(&mut self, name: &str) {
        RcType::make_mut(&mut self.tests).remove(name);
    }

    /// Adds a new global function.
    ///
    /// For details about functions have a look at [`functions`].  Note that
    /// functions and other global variables share the same namespace.
    pub fn add_function<F, Rv, Args>(&mut self, name: &'source str, f: F)
    where
        Rv: Into<Value>,
        F: functions::Function<Rv, Args>,
        Args: FunctionArgs,
    {
        self.add_global(name, functions::BoxedFunction::new(f).to_value());
    }

    /// Adds a global variable.
    pub fn add_global(&mut self, name: &'source str, value: Value) {
        RcType::make_mut(&mut self.globals).insert(name, value);
    }

    /// Removes a global function or variable by name.
    pub fn remove_global(&mut self, name: &str) {
        RcType::make_mut(&mut self.globals).remove(name);
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

    /// Finalizes a value.
    pub(crate) fn finalize(
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
