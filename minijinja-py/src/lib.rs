use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use minijinja::value::{Object, SeqObject, StructObject, Value};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySequence};

#[pyclass]
struct Environment {
    inner: minijinja::Environment<'static>,
}

struct DictLikeObject {
    inner: Py<PyDict>,
}

impl StructObject for DictLikeObject {
    fn get_field(&self, name: &str) -> Option<Value> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.get_item(name).map(wrap_value)
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
    inner: Py<PyList>,
}

impl SeqObject for ListLikeObject {
    fn get_item(&self, idx: usize) -> Option<Value> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.get_item(idx).ok().map(wrap_value)
        })
    }

    fn item_count(&self) -> usize {
        Python::with_gil(|py| self.inner.as_ref(py).len())
    }
}

struct DynamicObject {
    inner: Py<PyAny>,
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
    fn kind(&self) -> minijinja::value::ObjectKind<'_> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            if inner.downcast::<PySequence>().is_ok() {
                minijinja::value::ObjectKind::Seq(self)
            } else {
                minijinja::value::ObjectKind::Struct(self)
            }
        })
    }
}

impl SeqObject for DynamicObject {
    fn get_item(&self, idx: usize) -> Option<Value> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            if let Ok(seq) = inner.downcast::<PySequence>() {
                seq.get_item(idx).ok().map(wrap_value)
            } else {
                None
            }
        })
    }

    fn item_count(&self) -> usize {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.len().unwrap_or(0)
        })
    }
}

impl StructObject for DynamicObject {
    fn get_field(&self, name: &str) -> Option<Value> {
        Python::with_gil(|py| {
            let inner = self.inner.as_ref(py);
            inner.getattr(name).map(wrap_value).ok()
        })
    }
}

fn wrap_value(value: &PyAny) -> Value {
    if let Ok(dict) = value.cast_as::<PyDict>() {
        Value::from_struct_object(DictLikeObject { inner: dict.into() })
    } else if let Ok(list) = value.cast_as::<PyList>() {
        Value::from_seq_object(ListLikeObject { inner: list.into() })
    } else if value.is_none() {
        Value::from(())
    } else if let Ok(val) = value.extract::<i64>() {
        Value::from(val)
    } else if let Ok(val) = value.extract::<f64>() {
        Value::from(val)
    } else if let Ok(val) = value.extract::<bool>() {
        Value::from(val)
    } else if let Ok(val) = value.extract::<&str>() {
        Value::from(val)
    } else {
        Value::from_object(DynamicObject {
            inner: value.into(),
        })
    }
}

#[pymethods]
impl Environment {
    #[new]
    fn py_new(templates: BTreeMap<String, String>) -> PyResult<Self> {
        let mut env = minijinja::Environment::new();
        let mut source = minijinja::Source::new();
        for (name, value) in templates.into_iter() {
            source.add_template(name, value).map_err(convert_err)?;
        }
        env.set_source(source);
        Ok(Environment { inner: env })
    }

    #[args(ctx = "**")]
    pub fn render_template(&self, _template_name: &str, ctx: Option<&PyDict>) -> PyResult<String> {
        let tmpl = self
            .inner
            .get_template(_template_name)
            .map_err(convert_err)?;
        let ctx = Value::from_struct_object(DictLikeObject {
            inner: ctx.unwrap().into(),
        });
        tmpl.render(ctx).map_err(convert_err)
    }
}

fn convert_err(err: minijinja::Error) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

#[pymodule]
fn minijinja_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Environment>()?;
    Ok(())
}
