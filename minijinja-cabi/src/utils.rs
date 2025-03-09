use std::ffi::{c_char, CStr};
use std::{ptr, slice};

use minijinja::{Error, ErrorKind};

use crate::error::LAST_ERROR;

pub(crate) trait AbiResult {
    fn err_value() -> Self;
}

impl AbiResult for bool {
    fn err_value() -> Self {
        false
    }
}

impl<T> AbiResult for *mut T {
    fn err_value() -> Self {
        ptr::null_mut()
    }
}

impl AbiResult for u64 {
    fn err_value() -> Self {
        0
    }
}

impl AbiResult for i64 {
    fn err_value() -> Self {
        0
    }
}

impl AbiResult for f64 {
    fn err_value() -> Self {
        0.0
    }
}

impl AbiResult for () {
    fn err_value() -> Self {}
}

pub(crate) struct Scope;

impl Scope {
    pub unsafe fn get_str(&self, s: *const c_char) -> Result<&str, Error> {
        if s.is_null() {
            return Ok("");
        }
        unsafe { CStr::from_ptr(s) }
            .to_str()
            .map_err(|_| Error::new(ErrorKind::InvalidOperation, "expected valid utf-8"))
    }

    pub unsafe fn get_bytes(&self, b: *const c_char, length: u64) -> Vec<u8> {
        if b.is_null() || length == 0 {
            return Vec::<u8>::new();
        }
        slice::from_raw_parts(b as *const u8, length as usize).to_vec()
    }
}

pub(crate) fn catch<F: FnOnce(&Scope) -> Result<R, Error>, R: AbiResult>(f: F) -> R {
    LAST_ERROR.with_borrow_mut(|x| *x = None);
    match f(&Scope) {
        Ok(result) => result,
        Err(err) => {
            LAST_ERROR.with_borrow_mut(|x| *x = Some(err));
            R::err_value()
        }
    }
}
