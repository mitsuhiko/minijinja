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

#[wasm_bindgen(module = "/js/support.js")]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Error)]
    type TemplateError;

    #[wasm_bindgen(constructor)]
    fn new(message: &str, info: &js_sys::Object) -> TemplateError;
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

fn parse_error_kind(kind: &str) -> ErrorKind {
    match kind {
        "NonPrimitive" => ErrorKind::NonPrimitive,
        "NonKey" => ErrorKind::NonKey,
        "SyntaxError" => ErrorKind::SyntaxError,
        "TemplateNotFound" => ErrorKind::TemplateNotFound,
        "TooManyArguments" => ErrorKind::TooManyArguments,
        "MissingArgument" => ErrorKind::MissingArgument,
        "UnknownFilter" => ErrorKind::UnknownFilter,
        "UnknownTest" => ErrorKind::UnknownTest,
        "UnknownFunction" => ErrorKind::UnknownFunction,
        "UnknownMethod" => ErrorKind::UnknownMethod,
        "BadEscape" => ErrorKind::BadEscape,
        "UndefinedError" => ErrorKind::UndefinedError,
        "BadSerialization" => ErrorKind::BadSerialization,
        "BadInclude" => ErrorKind::BadInclude,
        "EvalBlock" => ErrorKind::EvalBlock,
        "CannotUnpack" => ErrorKind::CannotUnpack,
        "WriteFailure" => ErrorKind::WriteFailure,
        "OutOfFuel" => ErrorKind::OutOfFuel,
        "InvalidDelimiter" => ErrorKind::InvalidDelimiter,
        "UnknownBlock" => ErrorKind::UnknownBlock,
        _ => ErrorKind::InvalidOperation,
    }
}

fn get_string(value: &JsValue, key: &str) -> Option<String> {
    Reflect::get(value, &JsValue::from_str(key))
        .ok()
        .and_then(|x| x.as_string())
}

/// Creates a MiniJinja error for an exception thrown by JavaScript code.
///
/// A thrown `TemplateError` becomes an error of the same kind with the
/// detail (or message) of the error.  Other exceptions are reported as
/// invalid operation with the given context.
pub fn js_exception_error(context: &str, value: JsValue) -> Error {
    if value.is_instance_of::<TemplateError>() {
        let kind = get_string(&value, "kind").unwrap_or_default();
        let detail = get_string(&value, "detail")
            .or_else(|| get_string(&value, "message"))
            .unwrap_or_default();
        return Error::new(parse_error_kind(&kind), detail).with_source(JsException::new(value));
    }
    let exc = JsException::new(value);
    Error::new(ErrorKind::InvalidOperation, format!("{context}: {exc}")).with_source(exc)
}

fn set(obj: &js_sys::Object, key: &str, value: impl Into<JsValue>) {
    Reflect::set(obj, &JsValue::from_str(key), &value.into()).ok();
}

/// Converts a MiniJinja error into a JavaScript `TemplateError`.
pub fn to_js_error(err: Error) -> JsValue {
    let info = js_sys::Object::new();
    set(&info, "kind", format!("{:?}", err.kind()));
    set(&info, "detail", err.detail());
    set(&info, "templateName", err.name());
    set(&info, "line", err.line().map(|x| x as u32));
    set(&info, "templateSource", err.template_source());
    set(
        &info,
        "range",
        match err.range() {
            Some(range) => {
                let obj = js_sys::Object::new();
                set(&obj, "start", range.start as u32);
                set(&obj, "end", range.end as u32);
                JsValue::from(obj)
            }
            None => JsValue::UNDEFINED,
        },
    );

    let mut source: Option<&(dyn std::error::Error + 'static)> = err.source();
    while let Some(err) = source {
        if let Some(exc) = err.downcast_ref::<JsException>() {
            set(&info, "cause", &*exc.value);
            break;
        }
        source = err.source();
    }

    TemplateError::new(&format!("{err:#}"), &info).into()
}

/// Creates a plain JavaScript `Error` with the given message.
pub fn plain_js_error(message: &str) -> JsValue {
    js_sys::Error::new(message).into()
}
