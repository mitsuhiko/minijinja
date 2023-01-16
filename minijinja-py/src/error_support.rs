use std::cell::RefCell;

use minijinja::{Error, ErrorKind};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

thread_local! {
    static STASHED_ERROR: RefCell<Option<PyErr>> = RefCell::new(None);
}

pub fn to_minijinja_error(err: PyErr) -> Error {
    let msg = err.to_string();
    STASHED_ERROR.with(|stash| {
        *stash.borrow_mut() = Some(err);
    });
    Error::new(ErrorKind::TemplateNotFound, msg)
}

pub fn to_py_error(original_err: Error) -> PyErr {
    STASHED_ERROR.with(|stash| {
        stash
            .borrow_mut()
            .take()
            .unwrap_or_else(|| PyRuntimeError::new_err(format!("{:#}", original_err)))
    })
}
