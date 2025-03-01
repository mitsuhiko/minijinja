#![cfg(target_family = "wasm")]
#![allow(non_snake_case)]

extern crate wee_alloc;

use std::collections::BTreeMap;

use fragile::Fragile;
use js_sys::{Array, Function, Object};
use minijinja::{self as mj, Error, ErrorKind, Value};
use wasm_bindgen::prelude::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Represents a MiniJinja environment.
#[wasm_bindgen]
#[derive(Clone)]
pub struct Environment {
    inner: mj::Environment<'static>,
}

#[wasm_bindgen]
impl Environment {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let mut inner = mj::Environment::new();
        minijinja_contrib::add_to_environment(&mut inner);
        Self { inner }
    }

    /// Registers a new template by name and source.
    pub fn addTemplate(&mut self, name: &str, source: &str) -> Result<(), JsError> {
        self.inner
            .add_template_owned(name.to_string(), source.to_string())
            .map_err(convert_error)
    }

    /// Removes a template by name.
    pub fn removeTemplate(&mut self, name: &str) {
        self.inner.remove_template(name);
    }

    /// Clears all templates from the environment.
    pub fn clearTemplates(&mut self) {
        self.inner.clear_templates();
    }

    /// Renders a registered template by name with the given context.
    pub fn renderTemplate(&mut self, name: &str, ctx: JsValue) -> Result<String, JsError> {
        let ctx = js_to_mj_value(ctx)?;
        let t = self.inner.get_template(name).map_err(convert_error)?;
        t.render(ctx).map_err(convert_error)
    }

    /// Renders a string template with the given context.
    ///
    /// This is useful for one-off template rendering without registering the template.  The
    /// template is parsed and rendered immediately.
    pub fn renderStr(&mut self, source: &str, ctx: JsValue) -> Result<String, JsError> {
        let ctx = js_to_mj_value(ctx)?;
        self.inner.render_str(source, ctx).map_err(convert_error)
    }

    /// Like `renderStr` but with a named template for auto escape detection.
    pub fn renderNamedStr(
        &mut self,
        name: &str,
        source: &str,
        ctx: JsValue,
    ) -> Result<String, JsError> {
        let ctx = js_to_mj_value(ctx)?;
        self.inner
            .render_named_str(name, source, ctx)
            .map_err(convert_error)
    }

    /// Evaluates an expression with the given context.
    ///
    /// This is useful for evaluating expressions outside of templates.  The expression is
    /// parsed and evaluated immediately.
    pub fn evalExpr(&mut self, expr: &str, ctx: JsValue) -> Result<JsValue, JsError> {
        let ctx = js_to_mj_value(ctx)?;
        let e = self.inner.compile_expression(expr).map_err(convert_error)?;
        let result = e.eval(ctx).map_err(convert_error)?;
        serde_wasm_bindgen::to_value(&result).map_err(|err| JsError::new(&err.to_string()))
    }

    /// Registers a filter function.
    pub fn addFilter(&mut self, name: &str, func: Function) {
        self.inner
            .add_filter(name.to_string(), create_js_callback(func));
    }

    /// Registers a test function.
    pub fn addTest(&mut self, name: &str, func: Function) {
        self.inner
            .add_test(name.to_string(), create_js_callback(func));
    }

    /// Enables python compatibility.
    pub fn enablePyCompat(&mut self) {
        self.inner
            .set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
    }

    /// Enables or disables debug mode.
    #[wasm_bindgen(getter)]
    pub fn debug(&self) -> bool {
        self.inner.debug()
    }

    #[wasm_bindgen(setter)]
    pub fn set_debug(&mut self, yes: bool) {
        self.inner.set_debug(yes);
    }

    /// Enables or disables block trimming.
    #[wasm_bindgen(getter)]
    pub fn trimBlocks(&self) -> bool {
        self.inner.trim_blocks()
    }

    #[wasm_bindgen(setter)]
    pub fn set_trimBlocks(&mut self, yes: bool) {
        self.inner.set_trim_blocks(yes);
    }

    /// Enables or disables the lstrip blocks feature.
    #[wasm_bindgen(getter)]
    pub fn lstripBlocks(&self) -> bool {
        self.inner.lstrip_blocks()
    }

    #[wasm_bindgen(setter)]
    pub fn set_lstripBlocks(&mut self, yes: bool) {
        self.inner.set_lstrip_blocks(yes);
    }

    /// Enables or disables keeping of the final newline.
    #[wasm_bindgen(getter)]
    pub fn keepTrailingNewline(&self) -> bool {
        self.inner.keep_trailing_newline()
    }

    #[wasm_bindgen(setter)]
    pub fn set_keepTrailingNewline(&mut self, yes: bool) {
        self.inner.set_keep_trailing_newline(yes);
    }

    /// Reconfigures the behavior of undefined variables.
    #[wasm_bindgen(getter)]
    pub fn undefinedBehavior(&self) -> UndefinedBehavior {
        self.inner.undefined_behavior().into()
    }

    #[wasm_bindgen(setter)]
    pub fn set_undefinedBehavior(&mut self, value: UndefinedBehavior) -> Result<(), JsError> {
        self.inner.set_undefined_behavior(value.into());
        Ok(())
    }

    /// Configures the max-fuel for template evaluation.
    #[wasm_bindgen(getter)]
    pub fn fuel(&self) -> Option<u32> {
        self.inner.fuel().map(|x| x as u32)
    }

    #[wasm_bindgen(setter)]
    pub fn set_fuel(&mut self, value: Option<u32>) {
        self.inner.set_fuel(value.map(|x| x as u64));
    }

    /// Registers a value as global.
    #[wasm_bindgen]
    pub fn addGlobal(&mut self, name: &str, value: JsValue) -> Result<(), JsError> {
        self.inner
            .add_global(name.to_string(), js_to_mj_value(value)?);
        Ok(())
    }

    /// Removes a global again.
    #[wasm_bindgen]
    pub fn removeGlobal(&mut self, name: &str) {
        self.inner.remove_global(name);
    }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Debug)]
pub enum UndefinedBehavior {
    Strict = "strict",
    Chainable = "chainable",
    Lenient = "lenient",
    SemiStrict = "semi_strct",
}

impl From<mj::UndefinedBehavior> for UndefinedBehavior {
    fn from(value: mj::UndefinedBehavior) -> Self {
        match value {
            mj::UndefinedBehavior::Strict => UndefinedBehavior::Strict,
            mj::UndefinedBehavior::Chainable => UndefinedBehavior::Chainable,
            mj::UndefinedBehavior::Lenient => UndefinedBehavior::Lenient,
            mj::UndefinedBehavior::SemiStrict => UndefinedBehavior::SemiStrict,
            _ => unreachable!(),
        }
    }
}

impl From<UndefinedBehavior> for mj::UndefinedBehavior {
    fn from(value: UndefinedBehavior) -> Self {
        match value {
            UndefinedBehavior::Strict => mj::UndefinedBehavior::Strict,
            UndefinedBehavior::Chainable => mj::UndefinedBehavior::Chainable,
            UndefinedBehavior::Lenient => mj::UndefinedBehavior::Lenient,
            UndefinedBehavior::SemiStrict => mj::UndefinedBehavior::SemiStrict,
            _ => unreachable!(),
        }
    }
}

fn convert_error(err: minijinja::Error) -> JsError {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    JsError::new(&format!("{:#}", err))
}

fn js_to_mj_value(value: JsValue) -> Result<Value, JsError> {
    if value.is_function() {
        Ok(Value::from_function(create_js_callback(Function::from(
            value,
        ))))
    } else if value.is_array() {
        let arr = Array::from(&value);
        let mut rv = Vec::new();
        for i in 0..arr.length() {
            rv.push(js_to_mj_value(arr.get(i))?);
        }
        Ok(Value::from(rv))
    } else if value.is_object() {
        let obj = Object::from(value);
        let entries = Object::entries(&obj);
        let mut map = BTreeMap::new();
        for i in 0..entries.length() {
            let entry = Array::from(&entries.get(i));
            let key = entry.get(0);
            let value = entry.get(1);
            map.insert(js_to_mj_value(key)?, js_to_mj_value(value)?);
        }
        Ok(Value::from(map))
    } else {
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}

fn create_js_callback(func: Function) -> impl Fn(&[Value]) -> Result<Value, Error> {
    let fragile_func = Fragile::new(func);
    move |args: &[Value]| -> Result<Value, Error> {
        let values = js_sys::Array::new();
        for arg in args {
            values.push(&serde_wasm_bindgen::to_value(arg).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    format!("failed to convert argument: {}", err),
                )
            })?);
        }
        let func = fragile_func.get();
        let rv = func.apply(&JsValue::null(), &values).unwrap();
        let ctx: Value = js_to_mj_value(rv).map_err(|err| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("failed to convert result: {:?}", err),
            )
        })?;
        Ok(ctx)
    }
}
