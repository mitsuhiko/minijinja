use std::sync::Mutex;

use minijinja::value::{Rest, Value};
use minijinja::{context, Error, Source};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use crate::error_support::{to_minijinja_error, to_py_error};
use crate::typeconv::{to_minijinja_value, to_python_args, to_python_value, DictLikeObject};

/// Represents a MiniJinja environment.
#[pyclass]
pub struct Environment {
    inner: Mutex<minijinja::Environment<'static>>,
}

#[pymethods]
impl Environment {
    #[new]
    fn py_new() -> PyResult<Self> {
        Ok(Environment {
            inner: Mutex::new(minijinja::Environment::new()),
        })
    }

    /// Enables or disables debug mode.
    #[setter]
    pub fn set_debug(&self, value: bool) -> PyResult<()> {
        let mut env = self.inner.lock().unwrap();
        env.set_debug(value);
        Ok(())
    }

    /// Enables or disables debug mode.
    #[getter]
    pub fn get_debug(&self) -> PyResult<bool> {
        let env = self.inner.lock().unwrap();
        Ok(env.debug())
    }

    /// Registers a filter function.
    #[pyo3(text_signature = "(self, name, callback)")]
    pub fn add_filter(&self, name: &str, callback: &PyAny) -> PyResult<()> {
        if !callback.is_callable() {
            return Err(PyRuntimeError::new_err("expected callback"));
        }
        let callback: Py<PyAny> = callback.into();
        let mut env = self.inner.lock().unwrap();
        env.add_filter(
            name.to_string(),
            move |args: Rest<Value>| -> Result<Value, Error> {
                Python::with_gil(|py| {
                    let (py_args, py_kwargs) =
                        to_python_args(py, &args).map_err(to_minijinja_error)?;
                    let rv = callback
                        .call(py, py_args, py_kwargs)
                        .map_err(to_minijinja_error)?;
                    Ok(to_minijinja_value(rv.as_ref(py)))
                })
            },
        );
        Ok(())
    }

    /// Registers a test function.
    #[pyo3(text_signature = "(self, name, callback)")]
    pub fn add_test(&self, name: &str, callback: &PyAny) -> PyResult<()> {
        if !callback.is_callable() {
            return Err(PyRuntimeError::new_err("expected callback"));
        }
        let callback: Py<PyAny> = callback.into();
        let mut env = self.inner.lock().unwrap();
        env.add_test(
            name.to_string(),
            move |args: Rest<Value>| -> Result<bool, Error> {
                Python::with_gil(|py| {
                    let (py_args, py_kwargs) =
                        to_python_args(py, &args).map_err(to_minijinja_error)?;
                    let rv = callback
                        .call(py, py_args, py_kwargs)
                        .map_err(to_minijinja_error)?;
                    Ok(to_minijinja_value(rv.as_ref(py)).is_true())
                })
            },
        );
        Ok(())
    }

    /// Sets a loader function for the environment.
    ///
    /// The loader function is invoked with the name of the template to load.  If the
    /// template exists the source code of the template should be returned a string,
    /// otherwise `None` can be used to indicate that the template does not exist.
    #[pyo3(text_signature = "(self, callback)")]
    pub fn set_loader(&self, callback: &PyAny) -> PyResult<()> {
        if !callback.is_callable() {
            return Err(PyRuntimeError::new_err("expected callback"));
        }
        let callback: Py<PyAny> = callback.into();
        let s = Source::with_loader(move |name| {
            Python::with_gil(|py| {
                let callback = callback.as_ref(py);
                let rv = callback
                    .call1(PyTuple::new(py, [name]))
                    .map_err(to_minijinja_error)?;
                if rv.is_none() {
                    Ok(None)
                } else {
                    Ok(Some(rv.to_string()))
                }
            })
        });
        self.inner.lock().unwrap().set_source(s);
        Ok(())
    }

    /// Renders a template looked up from the loader.
    ///
    /// The first argument is the name of the template, all other arguments must be passed
    /// as keyword arguments and are pass as render context of the template.
    #[args(ctx = "**")]
    #[pyo3(text_signature = "(self, template_name, /, **ctx)")]
    pub fn render_template(&self, __template_name: &str, ctx: Option<&PyDict>) -> PyResult<String> {
        let env = self.inner.lock().unwrap();
        let tmpl = env.get_template(__template_name).map_err(to_py_error)?;
        let ctx = ctx
            .map(|ctx| Value::from_struct_object(DictLikeObject { inner: ctx.into() }))
            .unwrap_or_else(|| context!());
        tmpl.render(ctx).map_err(to_py_error)
    }

    /// Renders a template from a string
    ///
    /// The first argument is the source of the template, all other arguments must be passed
    /// as keyword arguments and are pass as render context of the template.
    #[args(ctx = "**")]
    #[pyo3(text_signature = "(self, source, /, **ctx)")]
    pub fn render_str(&self, __source: &str, ctx: Option<&PyDict>) -> PyResult<String> {
        let env = self.inner.lock().unwrap();
        let ctx = ctx
            .map(|ctx| Value::from_struct_object(DictLikeObject { inner: ctx.into() }))
            .unwrap_or_else(|| context!());
        env.render_str(__source, ctx).map_err(to_py_error)
    }

    /// Evaluates an expression with a given context.
    #[args(ctx = "**")]
    #[pyo3(text_signature = "(self, expr, /, **ctx)")]
    pub fn eval_expr(&self, __expression: &str, ctx: Option<&PyDict>) -> PyResult<Py<PyAny>> {
        let env = self.inner.lock().unwrap();
        let expr = env.compile_expression(__expression).map_err(to_py_error)?;
        let ctx = ctx
            .map(|ctx| Value::from_struct_object(DictLikeObject { inner: ctx.into() }))
            .unwrap_or_else(|| context!());
        to_python_value(expr.eval(ctx).map_err(to_py_error)?)
    }
}
