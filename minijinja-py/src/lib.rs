use pyo3::prelude::*;

mod environment;
mod error_support;
mod typeconv;

#[pymodule]
fn minijinja(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<environment::Environment>()?;
    Ok(())
}
