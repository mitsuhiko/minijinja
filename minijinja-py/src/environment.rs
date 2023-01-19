use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

use minijinja::value::{Rest, Value};
use minijinja::{context, AutoEscape, Error, Source, State};
use pyo3::conversion::AsPyPointer;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use crate::error_support::{report_unraisable, to_minijinja_error, to_py_error};
use crate::state::bind_state;
use crate::typeconv::{
    get_custom_autoescape, to_minijinja_value, to_python_args, to_python_value, DictLikeObject,
};

thread_local! {
    static CURRENT_ENV: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
}

/// Represents a MiniJinja environment.
#[pyclass(subclass, module = "minijinja._lowlevel")]
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

    /// Sets fuel
    #[setter]
    pub fn set_fuel(&self, value: Option<u64>) -> PyResult<()> {
        let mut env = self.inner.lock().unwrap();
        env.set_fuel(value);
        Ok(())
    }

    /// Enables or disables debug mode.
    #[getter]
    pub fn get_fuel(&self) -> PyResult<Option<u64>> {
        let env = self.inner.lock().unwrap();
        Ok(env.fuel())
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
            move |state: &State, args: Rest<Value>| -> Result<Value, Error> {
                Python::with_gil(|py| {
                    bind_state(state, || {
                        let (py_args, py_kwargs) = to_python_args(py, callback.as_ref(py), &args)
                            .map_err(to_minijinja_error)?;
                        let rv = callback
                            .call(py, py_args, py_kwargs)
                            .map_err(to_minijinja_error)?;
                        Ok(to_minijinja_value(rv.as_ref(py)))
                    })
                })
            },
        );
        Ok(())
    }

    /// Removes a filter function.
    #[pyo3(text_signature = "(self, name)")]
    pub fn remove_filter(&self, name: &str) -> PyResult<()> {
        self.inner.lock().unwrap().remove_filter(name);
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
            move |state: &State, args: Rest<Value>| -> Result<bool, Error> {
                Python::with_gil(|py| {
                    bind_state(state, || {
                        let (py_args, py_kwargs) = to_python_args(py, callback.as_ref(py), &args)
                            .map_err(to_minijinja_error)?;
                        let rv = callback
                            .call(py, py_args, py_kwargs)
                            .map_err(to_minijinja_error)?;
                        Ok(to_minijinja_value(rv.as_ref(py)).is_true())
                    })
                })
            },
        );
        Ok(())
    }

    /// Removes a test function.
    #[pyo3(text_signature = "(self, name)")]
    pub fn remove_test(&self, name: &str) -> PyResult<()> {
        self.inner.lock().unwrap().remove_test(name);
        Ok(())
    }

    fn add_function(&self, name: &str, callback: &PyAny) -> PyResult<()> {
        let callback: Py<PyAny> = callback.into();
        let mut env = self.inner.lock().unwrap();
        env.add_function(
            name.to_string(),
            move |state: &State, args: Rest<Value>| -> Result<Value, Error> {
                Python::with_gil(|py| {
                    bind_state(state, || {
                        let (py_args, py_kwargs) = to_python_args(py, callback.as_ref(py), &args)
                            .map_err(to_minijinja_error)?;
                        let rv = callback
                            .call(py, py_args, py_kwargs)
                            .map_err(to_minijinja_error)?;
                        Ok(to_minijinja_value(rv.as_ref(py)))
                    })
                })
            },
        );
        Ok(())
    }

    /// Registers a global
    #[pyo3(text_signature = "(self, name, value)")]
    pub fn add_global(&self, name: &str, value: &PyAny) -> PyResult<()> {
        if value.is_callable() {
            self.add_function(name, value)
        } else {
            self.inner
                .lock()
                .unwrap()
                .add_global(name.to_string(), to_minijinja_value(value));
            Ok(())
        }
    }

    /// Removes a global
    #[pyo3(text_signature = "(self, name)")]
    pub fn remove_global(&self, name: &str) -> PyResult<()> {
        self.inner.lock().unwrap().remove_global(name);
        Ok(())
    }

    /// Sets an auto escape callback.
    ///
    /// Note that because this interface in MiniJinja is infallible, the callback is
    /// not able to raise an error.
    #[setter]
    pub fn set_auto_escape_callback(&self, callback: &PyAny) -> PyResult<()> {
        if !callback.is_callable() {
            return Err(PyRuntimeError::new_err("expected callback"));
        }
        let callback: Py<PyAny> = callback.into();
        let mut env = self.inner.lock().unwrap();
        env.set_auto_escape_callback(move |name: &str| -> AutoEscape {
            Python::with_gil(|py| {
                let py_args = PyTuple::new(py, [name]);
                let rv = match callback.call(py, py_args, None) {
                    Ok(value) => value,
                    Err(err) => {
                        report_unraisable(py, err);
                        return AutoEscape::None;
                    }
                };
                let rv = rv.as_ref(py);
                if rv.is_none() {
                    return AutoEscape::None;
                }
                if let Ok(value) = rv.extract::<&str>() {
                    match value {
                        "html" => AutoEscape::Html,
                        "json" => AutoEscape::Json,
                        other => get_custom_autoescape(other),
                    }
                } else if let Ok(value) = rv.extract::<bool>() {
                    match value {
                        true => AutoEscape::Html,
                        false => AutoEscape::None,
                    }
                } else {
                    AutoEscape::None
                }
            })
        });
        Ok(())
    }

    /// Sets a loader function for the environment.
    ///
    /// The loader function is invoked with the name of the template to load.  If the
    /// template exists the source code of the template should be returned a string,
    /// otherwise `None` can be used to indicate that the template does not exist.
    #[setter]
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
    #[pyo3(signature = (template_name, /, **ctx))]
    pub fn render_template(
        slf: PyRef<'_, Self>,
        template_name: &str,
        ctx: Option<&PyDict>,
    ) -> PyResult<String> {
        bind_environment(slf.as_ptr(), || {
            let env = slf.inner.lock().unwrap();
            let tmpl = env.get_template(template_name).map_err(to_py_error)?;
            let ctx = ctx
                .map(|ctx| Value::from_struct_object(DictLikeObject { inner: ctx.into() }))
                .unwrap_or_else(|| context!());
            tmpl.render(ctx).map_err(to_py_error)
        })
    }

    /// Renders a template from a string
    ///
    /// The first argument is the source of the template, all other arguments must be passed
    /// as keyword arguments and are pass as render context of the template.
    #[pyo3(signature = (source, name=None, /, **ctx))]
    pub fn render_str(
        slf: PyRef<'_, Self>,
        source: &str,
        name: Option<&str>,
        ctx: Option<&PyDict>,
    ) -> PyResult<String> {
        bind_environment(slf.as_ptr(), || {
            let env = slf.inner.lock().unwrap();
            let ctx = ctx
                .map(|ctx| Value::from_struct_object(DictLikeObject { inner: ctx.into() }))
                .unwrap_or_else(|| context!());
            env.render_named_str(name.unwrap_or("<string>"), source, ctx)
                .map_err(to_py_error)
        })
    }

    /// Evaluates an expression with a given context.
    #[pyo3(signature = (expression, /, **ctx))]
    pub fn eval_expr(
        slf: PyRef<'_, Self>,
        expression: &str,
        ctx: Option<&PyDict>,
    ) -> PyResult<Py<PyAny>> {
        bind_environment(slf.as_ptr(), || {
            let env = slf.inner.lock().unwrap();
            let expr = env.compile_expression(expression).map_err(to_py_error)?;
            let ctx = ctx
                .map(|ctx| Value::from_struct_object(DictLikeObject { inner: ctx.into() }))
                .unwrap_or_else(|| context!());
            to_python_value(expr.eval(ctx).map_err(to_py_error)?)
        })
    }
}

pub fn with_environment<R, F: FnOnce(Py<Environment>) -> PyResult<R>>(f: F) -> PyResult<R> {
    Python::with_gil(|py| {
        CURRENT_ENV.with(|handle| {
            let ptr = handle.load(Ordering::Relaxed) as *mut _;
            match unsafe { Py::<Environment>::from_borrowed_ptr_or_opt(py, ptr) } {
                Some(env) => f(env),
                None => Err(PyRuntimeError::new_err(
                    "environment cannot be used outside of template render",
                )),
            }
        })
    })
}

/// Invokes a function with the state stashed away.
pub fn bind_environment<R, F: FnOnce() -> R>(envptr: *mut pyo3::ffi::PyObject, f: F) -> R {
    let old_handle = CURRENT_ENV
        .with(|handle| handle.swap(envptr as *const _ as *mut c_void, Ordering::Relaxed));
    let rv = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CURRENT_ENV.with(|handle| handle.store(old_handle, Ordering::Relaxed));
    match rv {
        Ok(rv) => rv,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
