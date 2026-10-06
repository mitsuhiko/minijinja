use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

use minijinja::value::{DynObject, Enumerator, Object, ObjectRepr, Tuple, Value, ValueKind};
use minijinja::{Error, State};

use pyo3::exceptions::{PyAttributeError, PyLookupError, PyTypeError};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PySequence, PyString, PyTuple, PyType};
use pyo3::{ffi, intern, prelude::*, IntoPyObjectExt};

use crate::attach::with_py;
use crate::error_support::{to_minijinja_error, to_py_error};
use crate::state::{bind_state, StateRef};

static MARK_SAFE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Public attributes of `dict` instances.
///
/// The attributes of the builtin `dict` type cannot be changed, so for exact
/// dicts the attribute fallback only needs to consider these.
const DICT_ATTRS: &[&str] = &[
    "clear",
    "copy",
    "fromkeys",
    "get",
    "items",
    "keys",
    "pop",
    "popitem",
    "setdefault",
    "update",
    "values",
];

/// Public attributes of `list` instances.
const LIST_ATTRS: &[&str] = &[
    "append", "clear", "copy", "count", "extend", "index", "insert", "pop", "remove", "reverse",
    "sort",
];

/// Maximum number of names kept in the per-thread name cache.
const NAME_CACHE_SIZE: usize = 1024;

/// Maximum length of names that are cached.
const NAME_CACHE_MAX_LEN: usize = 64;

thread_local! {
    static NAME_CACHE: RefCell<HashMap<Box<str>, Py<PyString>, BuildHasherDefault<FnvHasher>>> =
        RefCell::default();
}

/// Simple FNV-1a hasher for short attribute names.
#[derive(Default)]
struct FnvHasher(u64);

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 {
            0xcbf29ce484222325
        } else {
            self.0
        };
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.0 = hash;
    }
}

/// Returns a Python string for an attribute or key name.
///
/// Templates look up the same few names over and over again.  Creating a
/// fresh Python string for each of these lookups means allocating it and
/// computing its hash every single time.  Instead we keep a small cache of
/// strings per thread.  As the cached strings have their hash precomputed
/// this also speeds up the following dictionary lookups.
fn intern_name<'py>(py: Python<'py>, name: &str) -> Bound<'py, PyString> {
    if name.len() > NAME_CACHE_MAX_LEN {
        return PyString::new(py, name);
    }
    NAME_CACHE
        .try_with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(rv) = cache.get(name) {
                return rv.bind(py).clone();
            }
            if cache.len() >= NAME_CACHE_SIZE {
                cache.clear();
            }
            let rv = PyString::intern(py, name);
            cache.insert(name.into(), rv.clone().unbind());
            rv
        })
        .unwrap_or_else(|_| PyString::new(py, name))
}

fn is_safe_attr(name: &str) -> bool {
    !name.starts_with('_')
}

/// Checks if a type implements a specific type slot.
///
/// This lets us find out if an object supports a protocol without having to
/// trigger (and swallow) exceptions.
fn has_slot(ty: &Bound<'_, PyType>, slot: std::ffi::c_int) -> bool {
    // SAFETY: `PyType_GetSlot` works on all types from Python 3.10 onwards.
    unsafe { !ffi::PyType_GetSlot(ty.as_type_ptr(), slot).is_null() }
}

/// The shape of a wrapped Python object.
///
/// This is determined once when the object is wrapped so that the engine
/// can query it without having to call back into Python.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// An exact `dict`.
    Dict,
    /// An exact `list`.
    List,
    /// Something that implements `collections.abc.Sequence`.
    Seq,
    /// Something that looks like a mapping (`__getitem__` and `items`).
    Map,
    /// Something that can be iterated over.
    Iterable,
    /// Everything else.
    Plain,
}

impl Kind {
    fn of(value: &Bound<'_, PyAny>) -> (Kind, bool) {
        if value.is_exact_instance_of::<PyDict>() {
            return (Kind::Dict, true);
        }
        if value.is_exact_instance_of::<PyList>() {
            return (Kind::List, true);
        }
        let ty = value.get_type();
        let sq_item = has_slot(&ty, ffi::Py_sq_item);
        let subscriptable =
            sq_item || has_slot(&ty, ffi::Py_mp_subscript) || value.is_instance_of::<PyType>();
        let kind = if subscriptable && value.cast::<PySequence>().is_ok() {
            Kind::Seq
        } else if subscriptable && value.hasattr(intern!(value.py(), "items")).unwrap_or(false) {
            Kind::Map
        } else if sq_item || has_slot(&ty, ffi::Py_tp_iter) {
            Kind::Iterable
        } else {
            Kind::Plain
        };
        (kind, subscriptable)
    }
}

pub struct DynamicObject {
    inner: Py<PyAny>,
    kind: Kind,
    /// Indicates that `inner[key]` might succeed.
    subscriptable: bool,
}

impl DynamicObject {
    pub fn new(inner: &Bound<'_, PyAny>) -> DynamicObject {
        let (kind, subscriptable) = Kind::of(inner);
        DynamicObject {
            inner: inner.clone().unbind(),
            kind,
            subscriptable,
        }
    }

    /// Looks up an item or attribute on a generic Python object.
    fn get_value_slow(&self, py: Python<'_>, key: &Value) -> Option<Value> {
        let inner = self.inner.bind(py);
        if self.subscriptable {
            match inner.get_item(to_python_value_impl(py, key.clone()).ok()?) {
                Ok(value) => return Some(to_minijinja_value(&value)),
                Err(err) => {
                    if !(err.is_instance_of::<PyAttributeError>(py)
                        || err.is_instance_of::<PyLookupError>(py)
                        || err.is_instance_of::<PyTypeError>(py))
                    {
                        return Some(Value::from(to_minijinja_error(err)));
                    }
                }
            }
        }
        self.get_attr(py, key.as_str()?)
    }

    /// Looks up an attribute (and only an attribute).
    fn get_attr(&self, py: Python<'_>, name: &str) -> Option<Value> {
        if !is_safe_attr(name) {
            return None;
        }
        let inner = self.inner.bind(py);
        match inner.getattr(intern_name(py, name)) {
            Ok(rv) => Some(to_minijinja_value(&rv)),
            Err(err) => {
                if err.is_instance_of::<PyAttributeError>(py) {
                    None
                } else {
                    Some(Value::from(to_minijinja_error(err)))
                }
            }
        }
    }

    fn get_dict_item<'py>(
        &self,
        py: Python<'py>,
        key: impl IntoPyObject<'py>,
        attr: Option<&str>,
    ) -> Option<Value> {
        // SAFETY: the kind is only set to `Dict` for exact dicts.
        let dict = unsafe { self.inner.bind(py).cast_unchecked::<PyDict>() };
        match dict.get_item(key) {
            Ok(Some(value)) => return Some(to_minijinja_value(&value)),
            Ok(None) => {}
            // unhashable keys
            Err(err) if err.is_instance_of::<PyTypeError>(py) => {}
            Err(err) => return Some(Value::from(to_minijinja_error(err))),
        }
        match attr {
            Some(attr) if DICT_ATTRS.contains(&attr) => self.get_attr(py, attr),
            _ => None,
        }
    }

    fn get_list_item(&self, py: Python<'_>, key: &Value) -> Option<Value> {
        // SAFETY: the kind is only set to `List` for exact lists.
        let list = unsafe { self.inner.bind(py).cast_unchecked::<PyList>() };
        if let Some(idx) = key.as_i64() {
            let len = list.len() as i64;
            let idx = if idx < 0 { idx + len } else { idx };
            if idx >= 0 && idx < len {
                if let Ok(item) = list.get_item(idx as usize) {
                    return Some(to_minijinja_value(&item));
                }
            }
            None
        } else {
            match key.as_str() {
                Some(attr) if LIST_ATTRS.contains(&attr) => self.get_attr(py, attr),
                _ => None,
            }
        }
    }

    fn call_python(
        &self,
        py: Python<'_>,
        state: &mut State,
        callable: &Bound<'_, PyAny>,
        args: &[Value],
    ) -> Result<Value, Error> {
        bind_state(state, || {
            let (py_args, py_kwargs) =
                to_python_args(py, wants_state(callable), args).map_err(to_minijinja_error)?;
            Ok(to_minijinja_value(
                &callable
                    .call(py_args, py_kwargs.as_ref())
                    .map_err(to_minijinja_error)?,
            ))
        })
    }
}

impl fmt::Debug for DynamicObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        with_py(|py| write!(f, "{}", self.inner.bind(py)))
    }
}

impl Object for DynamicObject {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        match self.kind {
            Kind::Dict | Kind::Map => ObjectRepr::Map,
            Kind::List | Kind::Seq => ObjectRepr::Seq,
            Kind::Iterable => ObjectRepr::Iterable,
            Kind::Plain => ObjectRepr::Plain,
        }
    }

    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result
    where
        Self: Sized + 'static,
    {
        with_py(|py| write!(f, "{}", self.inner.bind(py)))
    }

    fn call(self: &Arc<Self>, state: &mut State, args: &[Value]) -> Result<Value, Error> {
        with_py(|py| self.call_python(py, state, self.inner.bind(py), args))
    }

    fn call_method(
        self: &Arc<Self>,
        state: &mut State,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        if !is_safe_attr(name) {
            return Err(Error::new(
                minijinja::ErrorKind::InvalidOperation,
                "insecure method call",
            ));
        }
        with_py(|py| {
            let method = self
                .inner
                .bind(py)
                .getattr(intern_name(py, name))
                .map_err(to_minijinja_error)?;
            self.call_python(py, state, &method, args)
        })
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        with_py(|py| match self.kind {
            Kind::Dict => {
                let py_key = to_python_value_impl(py, key.clone()).ok()?;
                self.get_dict_item(py, py_key, key.as_str())
            }
            Kind::List => self.get_list_item(py, key),
            _ => self.get_value_slow(py, key),
        })
    }

    fn get_value_by_str(self: &Arc<Self>, key: &str) -> Option<Value> {
        with_py(|py| match self.kind {
            Kind::Dict => self.get_dict_item(py, intern_name(py, key), Some(key)),
            Kind::List => self.get_list_item(py, &Value::from(key)),
            Kind::Plain if !self.subscriptable => self.get_attr(py, key),
            _ => self.get_value_slow(py, &Value::from(key)),
        })
    }

    fn custom_cmp(self: &Arc<Self>, other: &DynObject) -> Option<Ordering> {
        // Attention: this can violate the requirements of custom_cmp,
        // namely that it implements a total order.
        with_py(|py| {
            let self_inner = self.inner.bind(py);
            let other = other.downcast_ref::<DynamicObject>()?;
            let other_inner = other.inner.bind(py);
            self_inner.compare(other_inner).ok()
        })
    }

    fn is_true(self: &Arc<Self>) -> bool {
        with_py(|py| {
            let inner = self.inner.bind(py);
            inner.is_truthy().unwrap_or(true)
        })
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        match self.kind {
            Kind::Dict | Kind::List | Kind::Seq => with_py(|py| self.inner.bind(py).len().ok()),
            Kind::Plain => None,
            Kind::Map | Kind::Iterable => match self.enumerate() {
                Enumerator::Values(values) => Some(values.len()),
                _ => None,
            },
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        match self.kind {
            Kind::Plain => Enumerator::NonEnumerable,
            Kind::List | Kind::Seq => {
                with_py(|py| Enumerator::Seq(self.inner.bind(py).len().unwrap_or(0)))
            }
            Kind::Dict => with_py(|py| {
                // SAFETY: the kind is only set to `Dict` for exact dicts.
                let dict = unsafe { self.inner.bind(py).cast_unchecked::<PyDict>() };
                Enumerator::Values(dict.keys().iter().map(|x| to_minijinja_value(&x)).collect())
            }),
            Kind::Map | Kind::Iterable => with_py(|py| {
                let Ok(iter) = self.inner.bind(py).try_iter() else {
                    return Enumerator::NonEnumerable;
                };
                let mut values = Vec::new();
                for item in iter {
                    match item {
                        Ok(item) => values.push(to_minijinja_value(&item)),
                        Err(err) => {
                            // The error is yielded as an invalid value which fails
                            // the iteration when it's reached.  The length is not
                            // reported so that the error is not counted as an item.
                            values.push(Value::from(to_minijinja_error(err)));
                            return Enumerator::Iter(Box::new(UnknownLength(values.into_iter())));
                        }
                    }
                }
                Enumerator::Values(values)
            }),
        }
    }
}

/// Iterator wrapper that does not report a length.
struct UnknownLength<I>(I);

impl<I: Iterator> Iterator for UnknownLength<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Converts a Python int into a value.
fn int_to_value(value: &Bound<'_, PyAny>) -> Option<Value> {
    if let Ok(val) = value.extract::<i64>() {
        Some(Value::from(val))
    } else if let Ok(val) = value.extract::<u64>() {
        Some(Value::from(val))
    } else if let Ok(val) = value.extract::<i128>() {
        Some(Value::from(val))
    } else if let Ok(val) = value.extract::<u128>() {
        Some(Value::from(val))
    } else if let Ok(val) = value.extract::<f64>() {
        Some(Value::from(val))
    } else {
        None
    }
}

/// Converts an object that implements the number protocol.
fn number_to_value(value: &Bound<'_, PyAny>) -> Option<Value> {
    if let Ok(val) = value.extract::<bool>() {
        Some(Value::from(val))
    } else {
        int_to_value(value)
    }
}

pub fn to_minijinja_value(value: &Bound<'_, PyAny>) -> Value {
    // Fast paths for the exact builtin types.  These cannot have custom
    // behavior attached to them so we can avoid probing.
    if value.is_none() {
        return Value::from(());
    } else if let Ok(val) = value.cast_exact::<PyString>() {
        if let Ok(val) = val.to_str() {
            return Value::from(val);
        }
    } else if let Ok(val) = value.cast_exact::<PyBool>() {
        return Value::from(val.is_true());
    } else if value.is_exact_instance_of::<PyInt>() {
        if let Some(rv) = int_to_value(value) {
            return rv;
        }
    } else if let Ok(val) = value.cast_exact::<PyFloat>() {
        return Value::from(val.value());
    } else if value.is_exact_instance_of::<PyDict>() || value.is_exact_instance_of::<PyList>() {
        return Value::from_object(DynamicObject::new(value));
    }

    // Generic objects.  Only things implementing the number protocol can be
    // converted into numbers so avoid trying unless that's the case.
    // SAFETY: `PyNumber_Check` is infallible.
    if unsafe { ffi::PyNumber_Check(value.as_ptr()) } != 0 {
        if let Some(rv) = number_to_value(value) {
            return rv;
        }
    }

    if let Ok(tuple) = value.cast::<PyTuple>() {
        Value::from(Tuple::new(
            tuple.iter().map(|item| to_minijinja_value(&item)).collect(),
        ))
    } else if let Ok(val) = value.cast::<PyString>() {
        if let Ok(to_html) = value.getattr(intern!(value.py(), "__html__")) {
            if to_html.is_callable() {
                // TODO: if to_minijinja_value returns results we could
                // report the swallowed error of __html__.
                if let Ok(html) = to_html.call0() {
                    if let Ok(html) = html.cast::<PyString>() {
                        return Value::from_safe_string(html.to_string_lossy().into_owned());
                    }
                }
            }
        }
        Value::from(val.to_string_lossy())
    } else {
        Value::from_object(DynamicObject::new(value))
    }
}

pub fn to_python_value(value: Value) -> PyResult<Py<PyAny>> {
    with_py(|py| to_python_value_impl(py, value))
}

fn mark_string_safe(py: Python<'_>, value: &str) -> PyResult<Py<PyAny>> {
    let mark_safe: &Py<PyAny> = MARK_SAFE.get_or_try_init::<_, PyErr>(py, || {
        let module = py.import("minijinja._internal")?;
        Ok(module.getattr("mark_safe")?.into())
    })?;
    mark_safe.call1(py, (value,))
}

fn to_python_value_impl(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    // if we are holding a true dynamic object, we want to allow bidirectional
    // conversion.  That means that when passing the object back to Python we
    // extract the retained raw Python reference.
    if let Some(pyobj) = value.downcast_object_ref::<DynamicObject>() {
        return Ok(pyobj.inner.clone_ref(py));
    }

    if let Some(tuple) = value.downcast_object_ref::<Tuple>() {
        let items = tuple
            .iter()
            .cloned()
            .map(|value| to_python_value_impl(py, value))
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(PyTuple::new(py, items)?.into());
    }

    if let Some(obj) = value.as_object() {
        match obj.repr() {
            ObjectRepr::Plain => return obj.to_string().into_py_any(py),
            ObjectRepr::Map => {
                let rv = PyDict::new(py);
                if let Some(pair_iter) = obj.try_iter_pairs() {
                    for (key, value) in pair_iter {
                        rv.set_item(
                            to_python_value_impl(py, key)?,
                            to_python_value_impl(py, value)?,
                        )?;
                    }
                }
                return Ok(rv.into());
            }
            ObjectRepr::Seq | ObjectRepr::Iterable => {
                let rv = PyList::empty(py);
                if let Ok(iter) = value.try_iter() {
                    for value in iter.checked() {
                        let value = value.map_err(to_py_error)?;
                        rv.append(to_python_value_impl(py, value)?)?;
                    }
                }
                return Ok(rv.into());
            }
            _ => {}
        }
    }

    match value.kind() {
        ValueKind::Undefined | ValueKind::None => Ok(py.None()),
        ValueKind::Bool => Ok(value.is_true().into_py_any(py)?),
        ValueKind::Number => {
            if let Some(rv) = value.as_i64() {
                Ok(rv.into_py_any(py)?)
            } else if let Ok(rv) = TryInto::<u64>::try_into(value.clone()) {
                Ok(rv.into_py_any(py)?)
            } else if let Ok(rv) = TryInto::<i128>::try_into(value.clone()) {
                Ok(rv.into_py_any(py)?)
            } else if let Ok(rv) = TryInto::<u128>::try_into(value.clone()) {
                Ok(rv.into_py_any(py)?)
            } else if let Ok(rv) = TryInto::<f64>::try_into(value) {
                Ok(rv.into_py_any(py)?)
            } else {
                unreachable!()
            }
        }
        ValueKind::String => {
            if value.is_safe() {
                Ok(mark_string_safe(py, value.as_str().unwrap())?)
            } else {
                Ok(value.as_str().unwrap().into_py_any(py)?)
            }
        }
        ValueKind::Bytes => Ok(value.as_bytes().unwrap().into_py_any(py)?),
        kind => Err(to_py_error(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("object {kind} cannot roundtrip"),
        ))),
    }
}

/// Checks if a callable wants to be passed the state.
///
/// This is decided by the `__minijinja_pass_state__` attribute which is
/// set by the `pass_state` decorator.
pub fn wants_state(callable: &Bound<'_, PyAny>) -> bool {
    callable
        .getattr_opt(intern!(callable.py(), "__minijinja_pass_state__"))
        .ok()
        .flatten()
        .is_some_and(|x| x.is_truthy().unwrap_or(false))
}

pub fn to_python_args<'py>(
    py: Python<'py>,
    pass_state: bool,
    args: &[Value],
) -> PyResult<(Bound<'py, PyTuple>, Option<Bound<'py, PyDict>>)> {
    let mut py_args = Vec::with_capacity(args.len() + pass_state as usize);
    let mut py_kwargs = None;

    if pass_state {
        py_args.push(Bound::new(py, StateRef)?.into_any().unbind());
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
    let py_args = PyTuple::new(py, py_args)?;
    Ok((py_args, py_kwargs))
}
