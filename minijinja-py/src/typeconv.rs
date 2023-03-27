use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use minijinja::value::{Object, ObjectKind, SeqObject, StructObject, Value, ValueKind};
use minijinja::{AutoEscape, Error, State};

use once_cell::sync::OnceCell;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySequence, PyTuple};

use crate::error_support::to_minijinja_error;
use crate::state::{bind_state, StateRef};

static AUTO_ESCAPE_CACHE: Mutex<BTreeMap<String, AutoEscape>> = Mutex::new(BTreeMap::new());
static MARK_SAFE: OnceCell<Py<PyAny>> = OnceCell::new();

fn is_safe_attr(name: &str) -> bool {
    !name.starts_with('_')
}

pub struct DictLikeObject {
    pub inner: Py<PyDict>,
}

impl StructObject for DictLikeObject {
    fn get_field(&self, name: &str) -> Option<Value> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.get_item(name).map(to_minijinja_value)
        })
    }

    fn fields(&self) -> Vec<Arc<String>> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.keys().iter().map(|x| x.to_string().into()).collect()
        })
    }
}

struct ListLikeObject {
    inner: Py<PySequence>,
}

impl SeqObject for ListLikeObject {
    fn get_item(&self, idx: usize) -> Option<Value> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.get_item(idx).ok().map(to_minijinja_value)
        })
    }

    fn item_count(&self) -> usize {
        Python::with_gil(|py| self.inner.as_ref(py).len().unwrap_or(0))
    }
}

struct DynamicObject {
    inner: Py<PyAny>,
    sequencified: Option<Vec<Py<PyAny>>>,
}

impl fmt::Debug for DynamicObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self, f)
    }
}

impl fmt::Display for DynamicObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Python::with_gil(|py| write!(f, "{}", self.inner.as_ref(py)))
    }
}

impl Object for DynamicObject {
    fn kind(&self) -> ObjectKind<'_> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            if inner.downcast::<PySequence>().is_ok() || self.sequencified.is_some() {
                ObjectKind::Seq(self)
            } else {
                ObjectKind::Struct(self)
            }
        })
    }

    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        Python::with_gil(|py| -> Result<Value, Error> {
            bind_state(state, || {
                let inner = self.inner.as_ref(py);
                let (py_args, py_kwargs) =
                    to_python_args(py, inner, args).map_err(to_minijinja_error)?;
                Ok(to_minijinja_value(
                    inner.call(py_args, py_kwargs).map_err(to_minijinja_error)?,
                ))
            })
        })
    }

    fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        if !is_safe_attr(name) {
            return Err(Error::new(
                minijinja::ErrorKind::InvalidOperation,
                "insecure method call",
            ));
        }
        Python::with_gil(|py| -> Result<Value, Error> {
            bind_state(state, || {
                let inner = self.inner.as_ref(py);
                let (py_args, py_kwargs) =
                    to_python_args(py, inner, args).map_err(to_minijinja_error)?;
                Ok(to_minijinja_value(
                    inner
                        .call_method(name, py_args, py_kwargs)
                        .map_err(to_minijinja_error)?,
                ))
            })
        })
    }
}

impl SeqObject for DynamicObject {
    fn get_item(&self, idx: usize) -> Option<Value> {
        Python::with_gil(|py| {
            if let Some(ref seq) = self.sequencified {
                return seq.get(idx).map(|x| to_minijinja_value(x.as_ref(py)));
            }
            let inner = self.inner.as_ref(py);
            if let Ok(seq) = inner.downcast::<PySequence>() {
                seq.get_item(idx).ok().map(to_minijinja_value)
            } else {
                None
            }
        })
    }

    fn item_count(&self) -> usize {
        Python::with_gil(|py| {
            if let Some(ref seq) = self.sequencified {
                seq.len()
            } else {
                let inner = self.inner.as_ref(py);
                inner.len().unwrap_or(0)
            }
        })
    }
}

impl StructObject for DynamicObject {
    fn get_field(&self, name: &str) -> Option<Value> {
        if !is_safe_attr(name) {
            return None;
        }
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.getattr(name).map(to_minijinja_value).ok()
        })
    }
}

pub fn to_minijinja_value(value: &PyAny) -> Value {
    if let Ok(dict) = value.downcast::<PyDict>() {
        Value::from_struct_object(DictLikeObject { inner: dict.into() })
    } else if let Ok(tup) = value.downcast::<PyTuple>() {
        Value::from_seq_object(ListLikeObject {
            inner: tup.as_sequence().into(),
        })
    } else if let Ok(list) = value.downcast::<PyList>() {
        Value::from_seq_object(ListLikeObject {
            inner: list.as_sequence().into(),
        })
    } else if value.is_none() {
        Value::from(())
    } else if let Ok(val) = value.extract::<bool>() {
        Value::from(val)
    } else if let Ok(val) = value.extract::<i64>() {
        Value::from(val)
    } else if let Ok(val) = value.extract::<f64>() {
        Value::from(val)
    } else if let Ok(val) = value.extract::<&str>() {
        if let Ok(to_html) = value.getattr("__html__") {
            if to_html.is_callable() {
                // TODO: if to_minijinja_value returns results we could
                // report the swallowed error of __html__.
                if let Ok(html) = to_html.call0() {
                    if let Ok(val) = html.extract::<&str>() {
                        return Value::from_safe_string(val.into());
                    }
                }
            }
        }
        Value::from(val)
    } else {
        let mut sequencified = None;
        if let Ok(iter) = value.iter() {
            let mut seq = Vec::new();
            for value in iter.flatten() {
                seq.push(value.into());
            }
            sequencified = Some(seq);
        }
        Value::from_object(DynamicObject {
            inner: value.into(),
            sequencified,
        })
    }
}

pub fn to_python_value(value: Value) -> PyResult<Py<PyAny>> {
    Python::with_gil(|py| to_python_value_impl(py, value))
}

fn mark_string_safe(py: Python<'_>, value: &str) -> PyResult<Py<PyAny>> {
    let mark_safe: &Py<PyAny> = MARK_SAFE.get_or_try_init::<_, PyErr>(|| {
        let module = py.import("minijinja._internal")?;
        Ok(module.getattr("mark_safe")?.into())
    })?;
    mark_safe.call1(py, PyTuple::new(py, [value]))
}

fn to_python_value_impl(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    // if we are holding a true dynamic object, we want to allow bidirectional
    // conversion.  That means that when passing the object back to Python we
    // extract the retained raw Python reference.
    if let Some(pyobj) = value.downcast_object_ref::<DynamicObject>() {
        return Ok(pyobj.inner.clone());
    }

    if let Some(seq) = value.as_seq() {
        let rv = PyList::empty(py);
        for idx in 0..seq.item_count() {
            if let Some(item) = seq.get_item(idx) {
                rv.append(to_python_value_impl(py, item)?)?;
            } else {
                rv.append(py.None())?;
            }
        }
        Ok(rv.into())
    } else if let Some(s) = value.as_struct() {
        let rv = PyDict::new(py);
        for field in s.fields().into_iter() {
            if let Some(value) = s.get_field(&field) {
                rv.set_item(&field as &str, to_python_value_impl(py, value)?)?;
            }
        }
        Ok(rv.into())
    } else {
        match value.kind() {
            ValueKind::Undefined | ValueKind::None | ValueKind::Invalid => Ok(().into_py(py)),
            ValueKind::Bool => Ok(value.is_true().into_py(py)),
            ValueKind::Number => {
                if let Ok(rv) = TryInto::<i64>::try_into(value.clone()) {
                    Ok(rv.into_py(py))
                } else if let Ok(rv) = TryInto::<u64>::try_into(value.clone()) {
                    Ok(rv.into_py(py))
                } else if let Ok(rv) = TryInto::<f64>::try_into(value) {
                    Ok(rv.into_py(py))
                } else {
                    unreachable!()
                }
            }
            ValueKind::Char => {
                if let Ok(rv) = TryInto::<char>::try_into(value.clone()) {
                    Ok(rv.into_py(py))
                } else {
                    unreachable!()
                }
            }
            ValueKind::String => {
                if value.is_safe() {
                    Ok(mark_string_safe(py, value.as_str().unwrap())?)
                } else {
                    Ok(value.as_str().unwrap().into_py(py))
                }
            }
            ValueKind::Bytes => Ok(value.as_bytes().unwrap().into_py(py)),
            // this should be covered above
            ValueKind::Seq => unreachable!(),
            ValueKind::Map => {
                let rv = PyDict::new(py);
                if let Ok(iter) = value.try_iter() {
                    for k in iter {
                        if let Ok(v) = value.get_item(&k) {
                            rv.set_item(
                                to_python_value_impl(py, k)?,
                                to_python_value_impl(py, v)?,
                            )?;
                        }
                    }
                }
                Ok(rv.into())
            }
        }
    }
}

pub fn to_python_args<'py>(
    py: Python<'py>,
    callback: &PyAny,
    args: &[Value],
) -> PyResult<(&'py PyTuple, Option<&'py PyDict>)> {
    let mut py_args = Vec::new();
    let mut py_kwargs = None;

    if callback
        .getattr("__minijinja_pass_state__")
        .map_or(false, |x| x.is_true().unwrap_or(false))
    {
        py_args.push(Py::new(py, StateRef)?.to_object(py));
    }

    for arg in args {
        if arg.is_kwargs() {
            let kwargs = py_kwargs.get_or_insert_with(|| PyDict::new(py));
            if let Ok(iter) = arg.try_iter() {
                for k in iter {
                    if let Ok(v) = arg.get_item(&k) {
                        kwargs
                            .set_item(to_python_value_impl(py, k)?, to_python_value_impl(py, v)?)?;
                    }
                }
            }
        } else {
            py_args.push(to_python_value_impl(py, arg.clone())?);
        }
    }
    let py_args = PyTuple::new(py, py_args);
    Ok((py_args, py_kwargs))
}

pub fn get_custom_autoescape(value: &str) -> AutoEscape {
    let mut cache = AUTO_ESCAPE_CACHE.lock().unwrap();
    if let Some(rv) = cache.get(value).copied() {
        return rv;
    }
    let val = AutoEscape::Custom(Box::leak(value.to_string().into_boxed_str()));
    cache.insert(value.to_string(), val);
    val
}
