use pyo3::prelude::*;

mod environment;
mod error_support;
mod state;
mod typeconv;

#[pymodule]
fn _lowlevel(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<environment::Environment>()?;
    m.add_class::<state::StateRef>()?;
    m.add_class::<error_support::ErrorInfo>()?;
    m.add_function(wrap_pyfunction!(error_support::get_panic_info, m)?)?;
    crate::error_support::init_panic_hook();
    Ok(())
}
