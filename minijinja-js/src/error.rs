use std::error::Error as _;
use std::fmt;

use js_sys::Reflect;
use minijinja::{Error, ErrorKind};
use wasm_bindgen::prelude::*;

use crate::cell::JsCell;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_name = String)]
    fn js_string(value: &JsValue) -> Result<String, JsValue>;
}

/// Stringifies an arbitrary JavaScript value like `String(value)` does.
pub fn describe_js_value(value: &JsValue) -> String {
    js_string(value).unwrap_or_else(|_| "<unprintable value>".into())
}

/// A JavaScript exception attached to a MiniJinja error as source.
///
/// When the MiniJinja error is converted back into a JavaScript error, the
/// original exception is attached as `cause`.
pub struct JsException {
    message: String,
    value: JsCell<JsValue>,
}

impl JsException {
    pub fn new(value: JsValue) -> JsException {
        JsException {
            message: describe_js_value(&value),
            value: JsCell::new(value),
        }
    }
}

impl fmt::Debug for JsException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsException")
            .field("message", &self.message)
            .finish()
    }
}

impl fmt::Display for JsException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for JsException {}

/// Creates a MiniJinja error for an exception thrown by JavaScript code.
pub fn js_exception_error(context: &str, value: JsValue) -> Error {
    let exc = JsException::new(value);
    Error::new(ErrorKind::InvalidOperation, format!("{context}: {exc}")).with_source(exc)
}

/// Converts a MiniJinja error into a JavaScript `Error`.
pub fn to_js_error(err: Error) -> JsValue {
    let js_err = js_sys::Error::new(&format!("{err:#}"));
    let mut source: Option<&(dyn std::error::Error + 'static)> = err.source();
    while let Some(err) = source {
        if let Some(exc) = err.downcast_ref::<JsException>() {
            Reflect::set(&js_err, &JsValue::from_str("cause"), &exc.value).ok();
            break;
        }
        source = err.source();
    }
    js_err.into()
}

/// Creates a plain JavaScript `Error` with the given message.
pub fn plain_js_error(message: &str) -> JsValue {
    js_sys::Error::new(message).into()
}
