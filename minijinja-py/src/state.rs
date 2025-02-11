use minijinja::{AutoEscape, State};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::environment::{with_environment, Environment};
use crate::typeconv::{to_minijinja_value, to_python_value};

thread_local! {
    static CURRENT_STATE: AtomicPtr<c_void> = const { AtomicPtr::new(std::ptr::null_mut()) };
}

/// A reference to the current state.
#[pyclass(subclass, module = "minijinja._lowlevel", name = "State")]
pub struct StateRef;

#[pymethods]
impl StateRef {
    /// Returns a reference to the environment.
    #[getter]
    pub fn get_env(&self, py: Python<'_>) -> PyResult<Py<Environment>> {
        with_environment(py, Ok)
    }

    /// Returns the name of the template.
    #[getter]
    pub fn get_name(&self) -> PyResult<String> {
        with_state(|state| Ok(state.name().to_string()))
    }

    /// Returns the current auto escape flag
    #[getter]
    pub fn get_auto_escape(&self) -> PyResult<Option<&'static str>> {
        with_state(|state| {
            Ok(match state.auto_escape() {
                AutoEscape::None => None,
                AutoEscape::Html => Some("html"),
                AutoEscape::Json => Some("json"),
                AutoEscape::Custom(custom) => Some(custom),
                _ => None,
            })
        })
    }

    /// Returns the current block
    #[getter]
    pub fn get_current_block(&self) -> PyResult<Option<String>> {
        with_state(|state| Ok(state.current_block().map(|x| x.into())))
    }

    /// Looks up a variable in the context
    #[pyo3(text_signature = "(self, name)")]
    pub fn lookup(&self, name: &str) -> PyResult<Py<PyAny>> {
        with_state(|state| {
            state
                .lookup(name)
                .map(to_python_value)
                .unwrap_or_else(|| Ok(Python::with_gil(|py| py.None())))
        })
    }

    /// Looks up a temp by name.
    #[pyo3(signature = (name, default = None))]
    pub fn get_temp(&self, name: &str, default: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        with_state(|state| {
            let rv = state.get_temp(name);
            match rv {
                Some(rv) => to_python_value(rv),
                None => {
                    if let Some(default) = default {
                        let val = to_minijinja_value(default);
                        state.set_temp(name, val.clone());
                        to_python_value(val)
                    } else {
                        Ok(Python::with_gil(|py| py.None()))
                    }
                }
            }
        })
    }

    /// Sets a temp by name and returns the old value.
    #[pyo3(text_signature = "(self, name, value)")]
    pub fn set_temp(&self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        with_state(|state| {
            state
                .set_temp(name, to_minijinja_value(value))
                .map(to_python_value)
                .unwrap_or_else(|| Ok(Python::with_gil(|py| py.None())))
        })
    }
}

pub fn with_state<R, F: FnOnce(&State) -> PyResult<R>>(f: F) -> PyResult<R> {
    CURRENT_STATE.with(|handle| {
        match unsafe { (handle.load(Ordering::Relaxed) as *const State).as_ref() } {
            Some(state) => f(state),
            None => Err(PyRuntimeError::new_err(
                "state cannot be used outside of template render",
            )),
        }
    })
}

/// Invokes a function with the state stashed away.
pub fn bind_state<R, F: FnOnce() -> R>(state: &State, f: F) -> R {
    let old_handle = CURRENT_STATE
        .with(|handle| handle.swap(state as *const _ as *mut c_void, Ordering::Relaxed));
    let rv = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CURRENT_STATE.with(|handle| handle.store(old_handle, Ordering::Relaxed));
    match rv {
        Ok(rv) => rv,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
