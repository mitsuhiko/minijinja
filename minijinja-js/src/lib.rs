#![cfg(target_family = "wasm")]
#![allow(non_snake_case)]

use std::borrow::Cow;
use std::cell::{Ref, RefCell, RefMut};

use js_sys::Function;
use minijinja::value::{Rest, Value, ValueOrKwargs};
use minijinja::{self as mj, Error, ErrorKind};
use wasm_bindgen::prelude::*;

use crate::cell::JsCell;
use crate::error::{js_exception_error, plain_js_error, to_js_error};
use crate::value::{js_to_value, value_to_js, JsFunction};

mod cell;
mod error;
mod value;

#[wasm_bindgen(module = "/js/support.js")]
extern "C" {
    fn getSupportClasses() -> js_sys::Object;
}

/// Returns the support classes (`SafeString` and `TemplateError`).
///
/// This exists so that the package entry points can re-export the classes
/// that are used by the bindings.
#[wasm_bindgen(js_name = "__getSupportClasses", skip_typescript)]
pub fn get_support_classes() -> js_sys::Object {
    getSupportClasses()
}

#[wasm_bindgen(start)]
fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Represents a MiniJinja environment.
#[wasm_bindgen]
pub struct Environment {
    inner: RefCell<mj::Environment<'static>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    /// Borrows the environment for reading.
    fn env(&self) -> Result<Ref<'_, mj::Environment<'static>>, JsValue> {
        self.inner
            .try_borrow()
            .map_err(|_| plain_js_error("environment is currently being modified"))
    }

    /// Borrows the environment for modification.
    ///
    /// This fails if the environment is in use, for instance when a
    /// filter tries to reconfigure the environment while rendering.
    fn env_mut(&self) -> Result<RefMut<'_, mj::Environment<'static>>, JsValue> {
        self.inner.try_borrow_mut().map_err(|_| {
            plain_js_error("cannot modify the environment while it is in use (eg: while rendering)")
        })
    }

    /// Changes settings on the current syntax config.
    fn update_syntax(
        &self,
        f: impl FnOnce(&mut mj::syntax::SyntaxConfigBuilder),
    ) -> Result<(), JsValue> {
        let mut env = self.env_mut()?;
        let mut builder = env.syntax().to_builder();
        f(&mut builder);
        env.set_syntax(builder.build().map_err(to_js_error)?);
        Ok(())
    }
}

fn convert_ctx(ctx: Option<JsValue>) -> Result<Value, JsValue> {
    match ctx {
        Some(ctx) if !ctx.is_undefined() && !ctx.is_null() => {
            js_to_value(&ctx).map_err(to_js_error)
        }
        _ => Ok(mj::context! {}),
    }
}

fn js_callback(func: Function) -> impl Fn(Rest<ValueOrKwargs>) -> Result<Value, Error> {
    let func = JsFunction::new(func);
    move |args: Rest<ValueOrKwargs>| func.call(&args.into_values())
}

#[wasm_bindgen]
impl Environment {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let mut inner = mj::Environment::new();
        minijinja_contrib::add_to_environment(&mut inner);
        Self {
            inner: RefCell::new(inner),
        }
    }

    /// Registers a new template by name and source.
    pub fn addTemplate(&self, name: &str, source: &str) -> Result<(), JsValue> {
        self.env_mut()?
            .add_template_owned(name.to_string(), source.to_string())
            .map_err(to_js_error)
    }

    /// Removes a template by name.
    pub fn removeTemplate(&self, name: &str) -> Result<(), JsValue> {
        self.env_mut()?.remove_template(name);
        Ok(())
    }

    /// Clears all templates from the environment.
    pub fn clearTemplates(&self) -> Result<(), JsValue> {
        self.env_mut()?.clear_templates();
        Ok(())
    }

    /// Renders a registered template by name with the given context.
    pub fn renderTemplate(&self, name: &str, ctx: Option<JsValue>) -> Result<String, JsValue> {
        let ctx = convert_ctx(ctx)?;
        let env = self.env()?;
        let t = env.get_template(name).map_err(to_js_error)?;
        t.render(ctx).map_err(to_js_error)
    }

    /// Renders a string template with the given context.
    ///
    /// This is useful for one-off template rendering without registering the template.  The
    /// template is parsed and rendered immediately.
    pub fn renderStr(&self, source: &str, ctx: Option<JsValue>) -> Result<String, JsValue> {
        let ctx = convert_ctx(ctx)?;
        self.env()?.render_str(source, ctx).map_err(to_js_error)
    }

    /// Like `renderStr` but with a named template for auto escape detection.
    pub fn renderNamedStr(
        &self,
        name: &str,
        source: &str,
        ctx: Option<JsValue>,
    ) -> Result<String, JsValue> {
        let ctx = convert_ctx(ctx)?;
        self.env()?
            .render_named_str(name, source, ctx)
            .map_err(to_js_error)
    }

    /// Evaluates an expression with the given context.
    ///
    /// This is useful for evaluating expressions outside of templates.  The expression is
    /// parsed and evaluated immediately.
    pub fn evalExpr(&self, expr: &str, ctx: Option<JsValue>) -> Result<JsValue, JsValue> {
        let ctx = convert_ctx(ctx)?;
        let env = self.env()?;
        let e = env.compile_expression(expr).map_err(to_js_error)?;
        let result = e.eval(ctx).map_err(to_js_error)?;
        value_to_js(&result).map_err(to_js_error)
    }

    /// Registers a filter function.
    ///
    /// Keyword arguments are passed as a trailing object.
    pub fn addFilter(&self, name: &str, func: Function) -> Result<(), JsValue> {
        self.env_mut()?
            .add_filter(name.to_string(), js_callback(func));
        Ok(())
    }

    /// Registers a test function.
    ///
    /// Keyword arguments are passed as a trailing object.
    pub fn addTest(&self, name: &str, func: Function) -> Result<(), JsValue> {
        self.env_mut()?
            .add_test(name.to_string(), js_callback(func));
        Ok(())
    }

    /// Enables python compatibility.
    pub fn enablePyCompat(&self) -> Result<(), JsValue> {
        self.env_mut()?
            .set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
        Ok(())
    }

    /// Enables or disables debug mode.
    #[wasm_bindgen(getter)]
    pub fn debug(&self) -> Result<bool, JsValue> {
        Ok(self.env()?.debug())
    }

    #[wasm_bindgen(setter)]
    pub fn set_debug(&self, yes: bool) -> Result<(), JsValue> {
        self.env_mut()?.set_debug(yes);
        Ok(())
    }

    /// Enables or disables block trimming.
    #[wasm_bindgen(getter)]
    pub fn trimBlocks(&self) -> Result<bool, JsValue> {
        Ok(self.env()?.syntax().trim_blocks())
    }

    #[wasm_bindgen(setter)]
    pub fn set_trimBlocks(&self, yes: bool) -> Result<(), JsValue> {
        self.update_syntax(|syntax| {
            syntax.trim_blocks(yes);
        })
    }

    /// Enables or disables the lstrip blocks feature.
    #[wasm_bindgen(getter)]
    pub fn lstripBlocks(&self) -> Result<bool, JsValue> {
        Ok(self.env()?.syntax().lstrip_blocks())
    }

    #[wasm_bindgen(setter)]
    pub fn set_lstripBlocks(&self, yes: bool) -> Result<(), JsValue> {
        self.update_syntax(|syntax| {
            syntax.lstrip_blocks(yes);
        })
    }

    /// Enables or disables keeping of the final newline.
    #[wasm_bindgen(getter)]
    pub fn keepTrailingNewline(&self) -> Result<bool, JsValue> {
        Ok(self.env()?.syntax().keep_trailing_newline())
    }

    #[wasm_bindgen(setter)]
    pub fn set_keepTrailingNewline(&self, yes: bool) -> Result<(), JsValue> {
        self.update_syntax(|syntax| {
            syntax.keep_trailing_newline(yes);
        })
    }

    /// Reconfigures the behavior of undefined variables.
    #[wasm_bindgen(getter)]
    pub fn undefinedBehavior(&self) -> Result<UndefinedBehavior, JsValue> {
        Ok(match self.env()?.undefined_behavior() {
            mj::UndefinedBehavior::Strict => UndefinedBehavior::Strict,
            mj::UndefinedBehavior::Chainable => UndefinedBehavior::Chainable,
            mj::UndefinedBehavior::SemiStrict => UndefinedBehavior::SemiStrict,
            _ => UndefinedBehavior::Lenient,
        })
    }

    #[wasm_bindgen(setter)]
    pub fn set_undefinedBehavior(&self, value: UndefinedBehavior) -> Result<(), JsValue> {
        let value = match value {
            UndefinedBehavior::Strict => mj::UndefinedBehavior::Strict,
            UndefinedBehavior::Chainable => mj::UndefinedBehavior::Chainable,
            UndefinedBehavior::Lenient => mj::UndefinedBehavior::Lenient,
            UndefinedBehavior::SemiStrict => mj::UndefinedBehavior::SemiStrict,
            _ => {
                return Err(plain_js_error(
                    "invalid undefined behavior (expected one of \"strict\", \
                     \"chainable\", \"lenient\" or \"semi_strict\")",
                ))
            }
        };
        self.env_mut()?.set_undefined_behavior(value);
        Ok(())
    }

    /// Configures the max-fuel for template evaluation.
    #[wasm_bindgen(getter)]
    pub fn fuel(&self) -> Result<Option<u32>, JsValue> {
        Ok(self.env()?.fuel().map(|x| x.min(u32::MAX as u64) as u32))
    }

    #[wasm_bindgen(setter)]
    pub fn set_fuel(&self, value: Option<u32>) -> Result<(), JsValue> {
        self.env_mut()?.set_fuel(value.map(|x| x as u64));
        Ok(())
    }

    /// Registers a value as global.
    pub fn addGlobal(&self, name: &str, value: JsValue) -> Result<(), JsValue> {
        // convert before borrowing as conversion can invoke JavaScript code
        let value = js_to_value(&value).map_err(to_js_error)?;
        self.env_mut()?.add_global(name.to_string(), value);
        Ok(())
    }

    /// Removes a global again.
    pub fn removeGlobal(&self, name: &str) -> Result<(), JsValue> {
        self.env_mut()?.remove_global(name);
        Ok(())
    }

    /// Registers a synchronous template loader callback.
    ///
    /// The provided function is called with a template name and must return a
    /// string with the template source or `null`/`undefined` if the template
    /// does not exist. Errors thrown are propagated as MiniJinja errors.
    pub fn setLoader(&self, func: Function) -> Result<(), JsValue> {
        let func = JsCell::new(func);
        self.env_mut()?.set_loader(move |name| {
            let rv = func
                .call1(&JsValue::NULL, &JsValue::from_str(name))
                .map_err(|err| js_exception_error("template loader threw", err))?;
            if rv.is_undefined() || rv.is_null() {
                Ok(None)
            } else if let Some(source) = rv.as_string() {
                Ok(Some(source))
            } else {
                Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "loader must return a string or null/undefined",
                ))
            }
        });
        Ok(())
    }

    /// Sets a callback to join template paths (for relative includes/extends).
    ///
    /// The callback receives `(name, parent)` and should return a joined path string.
    /// If it throws or returns a non-string, the original `name` is used.
    pub fn setPathJoinCallback(&self, func: Function) -> Result<(), JsValue> {
        let func = JsCell::new(func);
        self.env_mut()?
            .set_path_join_callback(move |name, parent| -> Cow<'_, str> {
                match func.call2(
                    &JsValue::NULL,
                    &JsValue::from_str(name),
                    &JsValue::from_str(parent),
                ) {
                    Ok(rv) => match rv.as_string() {
                        Some(s) => Cow::Owned(s),
                        None => Cow::Borrowed(name),
                    },
                    Err(_) => Cow::Borrowed(name),
                }
            });
        Ok(())
    }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Debug)]
pub enum UndefinedBehavior {
    Strict = "strict",
    Chainable = "chainable",
    Lenient = "lenient",
    SemiStrict = "semi_strict",
}
