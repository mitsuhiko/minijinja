//! Conversion between JavaScript values and MiniJinja values.
//!
//! JavaScript to MiniJinja:
//!
//! - `undefined` becomes undefined, `null` becomes none
//! - `SafeString`s become safe strings
//! - booleans, strings and numbers map to their MiniJinja counterparts.  Numbers
//!   that are safe integers become integers.
//! - `BigInt` becomes a (128 bit) integer
//! - arrays, `Set`s and typed arrays become sequences
//! - `Uint8Array` and `ArrayBuffer` become bytes
//! - plain objects and `Map`s are copied into maps (preserving order)
//! - `Date` becomes an ISO 8601 string (or none if invalid)
//! - functions become callables
//! - all other objects (class instances, proxies, ...) are wrapped and
//!   accessed lazily.  Methods are invoked with the object as `this`.
//!
//! MiniJinja to JavaScript:
//!
//! - undefined becomes `undefined`, none becomes `null`
//! - integers outside of the safe integer range become `BigInt`
//! - sequences and iterables become arrays
//! - maps with only string keys (and keyword arguments) become plain objects,
//!   other maps become `Map`s
//! - safe strings become `SafeString`s
//! - bytes become `Uint8Array`
//! - wrapped JavaScript values are unwrapped again
//! - other objects are stringified

use std::fmt;
use std::sync::Arc;

use js_sys::{Array, ArrayBuffer, DataView, Date, Function, Map, Object, Reflect, Set, Uint8Array};
use minijinja::value::{Enumerator, Object as MjObject, ObjectRepr, Value, ValueKind};
use minijinja::{Error, ErrorKind, State};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::cell::JsCell;
use crate::error::{describe_js_value, js_exception_error};

/// Maximum nesting depth for converted values.
///
/// This protects against cyclic structures which would otherwise overflow
/// the stack.
const MAX_DEPTH: usize = 500;

/// `Number.MAX_SAFE_INTEGER`
const MAX_SAFE_INTEGER: f64 = 9007199254740991.0;

#[wasm_bindgen(module = "/js/support.js")]
extern "C" {
    #[wasm_bindgen(extends = js_sys::JsString)]
    pub type SafeString;

    #[wasm_bindgen(constructor)]
    pub fn new(value: &str) -> SafeString;
}

thread_local! {
    static OBJECT_PROTOTYPE: JsValue = Object::get_prototype_of(&Object::new()).into();
}

fn too_deep() -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        "value is nested too deeply (possibly a cyclic structure)",
    )
}

/// Converts a JavaScript value into a MiniJinja value.
pub fn js_to_value(value: &JsValue) -> Result<Value, Error> {
    js_to_value_impl(value, 0)
}

/// Converts a MiniJinja value into a JavaScript value.
pub fn value_to_js(value: &Value) -> Result<JsValue, Error> {
    value_to_js_impl(value, 0)
}

fn js_to_value_impl(value: &JsValue, depth: usize) -> Result<Value, Error> {
    if value.is_undefined() {
        return Ok(Value::UNDEFINED);
    } else if value.is_null() {
        return Ok(Value::from(()));
    } else if let Some(b) = value.as_bool() {
        return Ok(Value::from(b));
    } else if let Some(n) = value.as_f64() {
        return Ok(number_to_value(n));
    } else if let Some(s) = value.as_string() {
        return Ok(Value::from(s));
    } else if value.is_bigint() {
        return bigint_to_value(value);
    } else if let Some(func) = value.dyn_ref::<Function>() {
        return Ok(Value::from_object(JsFunction(JsCell::new(func.clone()))));
    } else if !value.is_object() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "cannot convert {} to a template value",
                describe_js_value(value)
            ),
        ));
    }

    if depth >= MAX_DEPTH {
        return Err(too_deep());
    }
    let depth = depth + 1;

    if value.is_instance_of::<SafeString>() {
        Ok(Value::from_safe_string(describe_js_value(value)))
    } else if Array::is_array(value) {
        convert_array(value.unchecked_ref(), depth)
    } else if is_plain_object(value) {
        let entries = Object::entries(value.unchecked_ref());
        let mut pairs = Vec::with_capacity(entries.length() as usize);
        for entry in entries.iter() {
            let entry: Array = entry.unchecked_into();
            pairs.push((
                Value::from(entry.get(0).as_string().unwrap_or_default()),
                js_to_value_impl(&entry.get(1), depth)?,
            ));
        }
        Ok(Value::from_pairs(pairs))
    } else if let Some(map) = value.dyn_ref::<Map>() {
        let mut pairs = Vec::with_capacity(map.size() as usize);
        for entry in Array::from(map).iter() {
            let entry: Array = entry.unchecked_into();
            pairs.push((
                js_to_value_impl(&entry.get(0), depth)?,
                js_to_value_impl(&entry.get(1), depth)?,
            ));
        }
        Ok(Value::from_pairs(pairs))
    } else if value.is_instance_of::<Set>() {
        convert_array(&Array::from(value), depth)
    } else if let Some(bytes) = value.dyn_ref::<Uint8Array>() {
        Ok(Value::from_bytes(bytes.to_vec()))
    } else if let Some(buffer) = value.dyn_ref::<ArrayBuffer>() {
        Ok(Value::from_bytes(Uint8Array::new(buffer).to_vec()))
    } else if ArrayBuffer::is_view(value) && !value.is_instance_of::<DataView>() {
        convert_array(&Array::from(value), depth)
    } else if let Some(date) = value.dyn_ref::<Date>() {
        if date.get_time().is_nan() {
            Ok(Value::from(()))
        } else {
            Ok(Value::from(String::from(date.to_iso_string())))
        }
    } else {
        Ok(Value::from_object(JsObject(JsCell::new(
            value.clone().unchecked_into(),
        ))))
    }
}

fn convert_array(arr: &Array, depth: usize) -> Result<Value, Error> {
    arr.iter()
        .map(|item| js_to_value_impl(&item, depth))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::from)
}

fn is_plain_object(value: &JsValue) -> bool {
    let proto = Object::get_prototype_of(value);
    let proto: &JsValue = &proto;
    proto.is_null() || OBJECT_PROTOTYPE.with(|object_proto| object_proto == proto)
}

fn number_to_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() <= MAX_SAFE_INTEGER {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}

fn bigint_to_value(value: &JsValue) -> Result<Value, Error> {
    if let Ok(v) = i64::try_from(value.clone()) {
        Ok(Value::from(v))
    } else if let Ok(v) = i128::try_from(value.clone()) {
        Ok(Value::from(v))
    } else if let Ok(v) = u128::try_from(value.clone()) {
        Ok(Value::from(v))
    } else {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "BigInt is out of range for template values",
        ))
    }
}

fn value_to_js_impl(value: &Value, depth: usize) -> Result<JsValue, Error> {
    match value.kind() {
        ValueKind::Undefined => return Ok(JsValue::UNDEFINED),
        ValueKind::None => return Ok(JsValue::NULL),
        ValueKind::Bool => return Ok(JsValue::from_bool(value.is_true())),
        ValueKind::Number => return Ok(number_to_js(value)),
        ValueKind::String => {
            let s = value.as_str().unwrap_or_default();
            return Ok(if value.is_safe() {
                SafeString::new(s).into()
            } else {
                JsValue::from_str(s)
            });
        }
        ValueKind::Bytes => {
            return Ok(Uint8Array::from(value.as_bytes().unwrap_or_default()).into());
        }
        ValueKind::Invalid => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("cannot convert invalid value: {value}"),
            ));
        }
        _ => {}
    }

    if let Some(obj) = value.downcast_object_ref::<JsObject>() {
        return Ok(JsValue::from(&**obj.0));
    } else if let Some(func) = value.downcast_object_ref::<JsFunction>() {
        return Ok(JsValue::from(&**func.0));
    }

    if depth >= MAX_DEPTH {
        return Err(too_deep());
    }
    let depth = depth + 1;

    match value.kind() {
        ValueKind::Seq | ValueKind::Iterable => {
            let rv = Array::new();
            for item in value.try_iter()? {
                rv.push(&value_to_js_impl(&item, depth)?);
            }
            Ok(rv.into())
        }
        ValueKind::Map => {
            let mut all_string_keys = true;
            let mut pairs = Vec::new();
            for key in value.try_iter()? {
                let item = value.get_item(&key)?;
                all_string_keys &= key.kind() == ValueKind::String;
                pairs.push((
                    value_to_js_impl(&key, depth)?,
                    value_to_js_impl(&item, depth)?,
                ));
            }
            if all_string_keys {
                // Object.fromEntries defines data properties so keys such as
                // `__proto__` do not invoke setters.
                let entries = Array::new();
                for (key, item) in &pairs {
                    entries.push(&Array::of2(key, item));
                }
                Object::from_entries(&entries)
                    .map(Into::into)
                    .map_err(|err| js_exception_error("failed to create object", err))
            } else {
                let map = Map::new();
                for (key, item) in &pairs {
                    map.set(key, item);
                }
                Ok(map.into())
            }
        }
        _ => Ok(JsValue::from_str(&value.to_string())),
    }
}

fn number_to_js(value: &Value) -> JsValue {
    if value.is_integer() {
        if let Ok(v) = i128::try_from(value.clone()) {
            if v.unsigned_abs() <= MAX_SAFE_INTEGER as u128 {
                JsValue::from_f64(v as f64)
            } else {
                JsValue::from(v)
            }
        } else {
            JsValue::from(u128::try_from(value.clone()).unwrap_or_default())
        }
    } else {
        JsValue::from_f64(f64::try_from(value.clone()).unwrap_or(f64::NAN))
    }
}

/// Invokes a JavaScript function with MiniJinja arguments.
///
/// Keyword arguments are passed as a trailing plain object.
pub fn call_js_function(func: &Function, this: &JsValue, args: &[Value]) -> Result<Value, Error> {
    let js_args = Array::new();
    for arg in args {
        js_args.push(&value_to_js(arg)?);
    }
    let rv = func
        .apply(this, &js_args)
        .map_err(|err| js_exception_error("JavaScript function threw", err))?;
    js_to_value(&rv)
}

/// A JavaScript function exposed to the template engine.
pub struct JsFunction(JsCell<Function>);

impl JsFunction {
    pub fn new(func: Function) -> JsFunction {
        JsFunction(JsCell::new(func))
    }

    pub fn call(&self, args: &[Value]) -> Result<Value, Error> {
        call_js_function(&self.0, &JsValue::UNDEFINED, args)
    }
}

impl fmt::Debug for JsFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {}>", String::from(self.0.name()))
    }
}

impl MjObject for JsFunction {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }

    fn call(self: &Arc<Self>, _state: &mut State<'_, '_>, args: &[Value]) -> Result<Value, Error> {
        JsFunction::call(self, args)
    }
}

/// A JavaScript object that is accessed lazily.
pub struct JsObject(JsCell<Object>);

impl fmt::Debug for JsObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<object>")
    }
}

impl MjObject for JsObject {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let key = match key.as_str() {
            Some(key) => JsValue::from_str(key),
            None => JsValue::from_f64(key.as_i64()? as f64),
        };
        match Reflect::get(&self.0, &key) {
            Ok(rv) => {
                if rv.is_undefined() && !Reflect::has(&self.0, &key).unwrap_or(false) {
                    None
                } else {
                    Some(js_to_value(&rv).unwrap_or_else(Value::from))
                }
            }
            Err(err) => Some(Value::from(js_exception_error(
                "failed to read property",
                err,
            ))),
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Values(
            Object::keys(&self.0)
                .iter()
                .filter_map(|key| key.as_string())
                .map(Value::from)
                .collect(),
        )
    }

    fn is_true(self: &Arc<Self>) -> bool {
        true
    }

    fn call_method(
        self: &Arc<Self>,
        _state: &mut State<'_, '_>,
        method: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let func = Reflect::get(&self.0, &JsValue::from_str(method))
            .map_err(|err| js_exception_error("failed to read property", err))?;
        match func.dyn_ref::<Function>() {
            Some(func) => call_js_function(func, &self.0, args),
            None => Err(Error::from(ErrorKind::UnknownMethod)),
        }
    }
}
