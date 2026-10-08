//! Exposes the engine state to JavaScript callbacks.
//!
//! Callbacks marked with `passState` receive a `State` object as first
//! argument.  The object refers to the engine state on the stack and is only
//! valid while the callback runs.  Afterwards all methods throw.
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

use minijinja::value::Value;
use minijinja::{self as mj, AutoEscape};
use wasm_bindgen::prelude::*;

use crate::error::{plain_js_error, to_js_error};
use crate::value::{js_to_value, value_to_js};
use crate::UndefinedBehavior;

/// The state of the engine during a callback.
///
/// The state is only valid while the callback that received it runs.
#[wasm_bindgen(js_name = State)]
pub struct JsState {
    ptr: Rc<Cell<*mut c_void>>,
}

/// Invalidates the state when dropped.
pub struct StateGuard {
    ptr: Rc<Cell<*mut c_void>>,
}

impl Drop for StateGuard {
    fn drop(&mut self) {
        self.ptr.set(ptr::null_mut());
    }
}

/// Creates a JavaScript state object for the duration of the returned guard.
pub fn bind_state(state: &mut mj::State<'_, '_>) -> (StateGuard, JsValue) {
    let ptr = Rc::new(Cell::new(state as *mut mj::State<'_, '_> as *mut c_void));
    (
        StateGuard { ptr: ptr.clone() },
        JsValue::from(JsState { ptr }),
    )
}

impl JsState {
    fn with<R>(
        &self,
        f: impl FnOnce(&mut mj::State<'_, '_>) -> Result<R, JsValue>,
    ) -> Result<R, JsValue> {
        let ptr = self.ptr.get();
        if ptr.is_null() {
            return Err(plain_js_error(
                "the state can only be used while the callback that received it runs",
            ));
        }
        // SAFETY: the pointer is only set while the callback runs that received
        // this object (see `StateGuard`) and the engine does not use the state
        // during that time.  The closure cannot leak references as it only
        // returns owned values.
        let state = unsafe { &mut *(ptr as *mut mj::State<'_, '_>) };
        f(state)
    }
}

fn convert_args(args: &[JsValue]) -> Result<Vec<Value>, JsValue> {
    args.iter()
        .map(|arg| js_to_value(arg).map_err(to_js_error))
        .collect()
}

#[wasm_bindgen(js_class = State)]
impl JsState {
    /// The name of the template that is rendered.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> Result<String, JsValue> {
        self.with(|state| Ok(state.name().to_string()))
    }

    /// The current auto escape mode (`"html"`, `"json"`, `"none"` or the
    /// name of a custom format).
    #[wasm_bindgen(getter)]
    pub fn autoEscape(&self) -> Result<String, JsValue> {
        self.with(|state| {
            Ok(match state.auto_escape() {
                AutoEscape::Html => "html".to_string(),
                AutoEscape::Json => "json".to_string(),
                AutoEscape::Custom(name) => name.to_string(),
                _ => "none".to_string(),
            })
        })
    }

    /// The undefined behavior of the environment.
    #[wasm_bindgen(getter)]
    pub fn undefinedBehavior(&self) -> Result<UndefinedBehavior, JsValue> {
        self.with(|state| Ok(state.undefined_behavior().into()))
    }

    /// The name of the block that is rendered, if any.
    #[wasm_bindgen(getter)]
    pub fn currentBlock(&self) -> Result<Option<String>, JsValue> {
        self.with(|state| Ok(state.current_block().map(|x| x.to_string())))
    }

    /// Looks up a variable by name (including globals).
    ///
    /// Returns `undefined` if the variable does not exist.
    pub fn lookup(&self, name: &str) -> Result<JsValue, JsValue> {
        self.with(|state| match state.lookup(name) {
            Some(value) => value_to_js(&value).map_err(to_js_error),
            None => Ok(JsValue::UNDEFINED),
        })
    }

    /// Applies a filter by name to the given arguments.
    #[wasm_bindgen(variadic)]
    pub fn applyFilter(&self, name: &str, args: Vec<JsValue>) -> Result<JsValue, JsValue> {
        let args = convert_args(&args)?;
        self.with(|state| {
            let rv = state.apply_filter(name, &args).map_err(to_js_error)?;
            value_to_js(&rv).map_err(to_js_error)
        })
    }

    /// Performs a test by name on the given arguments.
    #[wasm_bindgen(variadic)]
    pub fn performTest(&self, name: &str, args: Vec<JsValue>) -> Result<bool, JsValue> {
        let args = convert_args(&args)?;
        self.with(|state| state.perform_test(name, &args).map_err(to_js_error))
    }
}
