//! An extension package to MiniJinja that allows stack borrows.
//!
//! When implementing dynamic objects for MiniJinja lifetimes can quickly
//! cause issues as it won't be possible to pass borrowed values to the
//! template.  This crate allows you to get handles to values.  These
//! handles are designed to forward to [`Object`], [`SeqObject`] and
//! [`StructObject`] automatically.
//!
//! ```
//! use minijinja::value::{StructObject, Value};
//! use minijinja::{context, Environment};
//! use minijinja_stack_ref::stack_token;
//!
//! struct State {
//!     version: &'static str,
//! }
//!
//! impl StructObject for State {
//!     fn get_field(&self, field: &str) -> Option<Value> {
//!         match field {
//!             "version" => Some(Value::from(self.version)),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! let mut env = Environment::new();
//! env.add_template(
//!     "info",
//!     "app version: {{ state.version }}"
//! )
//! .unwrap();
//!
//! let tmpl = env.get_template("info").unwrap();
//! let state = State {
//!     version: env!("CARGO_PKG_VERSION"),
//! };
//!
//! // a stack token is created with the stack_token! macro.  After the
//! // creation of it, the utility methods on it can be used to get
//! // references enclosed in values to it.
//! stack_token!(scope);
//!
//! println!("{}", tmpl.render(context! {
//!     state => scope.struct_object_ref(&state),
//! }).unwrap());
//! ```
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;
use std::mem::transmute;
use std::sync::atomic::{AtomicU64, Ordering};

use minijinja::value::{Object, ObjectKind, SeqObject, StructObject, Value};
use minijinja::{Error, State};

static STACK_SCOPE_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static STACK_SCOPE_IS_VALID: RefCell<HashSet<u64>> = RefCell::default();
}

/// A handle to an enclosed value.
///
/// For as long as the [`StackToken`] is still valid access to the
/// reference can be temporarily fetched via the [`with`](Self::with)
/// method.  Doing so after the scope is gone, this will panic on all
/// operations.
///
/// To check if a handle is still valid [`is_valid`](Self::is_valid)
/// can be used.
///
/// A stack handle implements the underlying object protocols from
/// MiniJinja.
pub struct StackHandle<T> {
    ptr: *const T,
    id: u64,
}

unsafe impl<T> Send for StackHandle<T> {}
unsafe impl<T> Sync for StackHandle<T> {}

impl<T> StackHandle<T> {
    /// Checks if the handle is still valid.
    #[inline]
    pub fn is_valid(handle: &StackHandle<T>) -> bool {
        STACK_SCOPE_IS_VALID.with(|valid_ids| valid_ids.borrow().contains(&handle.id))
    }

    /// Invokes a function with the resolved reference.
    ///
    /// # Panics
    ///
    /// This method panics if the handle is not valid.
    pub fn with<F: FnOnce(&T) -> R, R>(&self, f: F) -> R {
        assert!(StackHandle::is_valid(self), "stack is gone");
        f(unsafe { &*self.ptr as &T })
    }
}

impl<T: SeqObject + Send + Sync + 'static> SeqObject for StackHandle<T> {
    fn get_item(&self, idx: usize) -> Option<Value> {
        self.with(|val| val.get_item(idx))
    }

    fn item_count(&self) -> usize {
        self.with(|val| val.item_count())
    }
}

impl<T: StructObject + Send + Sync + 'static> StructObject for StackHandle<T> {
    fn get_field(&self, idx: &str) -> Option<Value> {
        self.with(|val| val.get_field(idx))
    }

    fn fields(&self) -> Box<dyn Iterator<Item = Cow<'static, str>> + '_> {
        self.with(|val| {
            // TODO: can the collection into the vector be avoided?
            Box::new(val.fields().collect::<Vec<_>>().into_iter())
        })
    }

    fn field_count(&self) -> usize {
        self.with(|val| val.field_count())
    }
}

impl<T: Object> fmt::Debug for StackHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|val| fmt::Debug::fmt(val, f))
    }
}

impl<T: Object> fmt::Display for StackHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|val| fmt::Display::fmt(val, f))
    }
}

impl<T: Object> Object for StackHandle<T> {
    fn kind(&self) -> ObjectKind<'_> {
        self.with(|val| match val.kind() {
            ObjectKind::Plain => ObjectKind::Plain,
            ObjectKind::Seq(_) => {
                ObjectKind::Seq(unsafe { transmute::<_, &StackHandleProxy<T>>(self) })
            }
            ObjectKind::Struct(_) => {
                ObjectKind::Struct(unsafe { transmute::<_, &StackHandleProxy<T>>(self) })
            }
            _ => unimplemented!(),
        })
    }

    fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        self.with(|val| val.call_method(state, name, args))
    }

    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        self.with(|val| val.call(state, args))
    }
}

#[repr(transparent)]
struct StackHandleProxy<T: Object>(StackHandle<T>);

macro_rules! unwrap_kind {
    ($val:expr, $pat:path) => {
        if let $pat(rv) = $val.kind() {
            rv
        } else {
            unreachable!("object changed shape")
        }
    };
}

impl<T: Object> SeqObject for StackHandleProxy<T> {
    fn get_item(&self, idx: usize) -> Option<Value> {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Seq).get_item(idx))
    }

    fn item_count(&self) -> usize {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Seq).item_count())
    }
}

impl<T: Object> StructObject for StackHandleProxy<T> {
    fn get_field(&self, name: &str) -> Option<Value> {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Struct).get_field(name))
    }

    fn fields(&self) -> Box<dyn Iterator<Item = Cow<'static, str>> + '_> {
        self.0.with(|val| {
            Box::new(
                unwrap_kind!(val, ObjectKind::Struct)
                    .fields()
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        })
    }

    fn field_count(&self) -> usize {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Struct).field_count())
    }
}

/// A token on the stack for convenient temporary borrowing.
pub struct StackToken {
    id: u64,
    unset: bool,
    _marker: PhantomData<*const ()>,
}

impl StackToken {
    /// Creates a stack token.
    ///
    /// # Safety
    ///
    /// This must never be called directly, the only permissible caller is the
    /// [`stack_token`] macro which places it on the stack and immediately creates
    /// a reference to it.  Because of how the value is constructed the dtor is
    /// guaranteed to run and can clean up the reference in the TLS.
    #[doc(hidden)]
    pub unsafe fn __private_new() -> StackToken {
        let id = STACK_SCOPE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let unset = STACK_SCOPE_IS_VALID.with(|valid_ids| valid_ids.borrow_mut().insert(id));
        StackToken {
            id,
            unset,
            _marker: PhantomData,
        }
    }

    /// Creates a [`StackHandle`] to a value with at least the scope's lifetime.
    pub fn handle<'env, T: 'env>(&'env self, value: &'env T) -> StackHandle<T> {
        StackHandle {
            ptr: value as *const T,
            id: self.id,
        }
    }

    /// Creates a [`Value`] from a borrowed [`Object`].
    ///
    /// This is equivalent to `Value::from_object(self.handle(value))`.
    pub fn object_ref<'env, T: Object>(&'env self, value: &'env T) -> Value {
        Value::from_object(self.handle(value))
    }

    /// Creates a [`Value`] from a borrowed [`SeqObject`].
    ///
    /// This is equivalent to `Value::from_seq_object(self.handle(value))`.
    pub fn seq_object_ref<'env, T: SeqObject + 'static>(&'env self, value: &'env T) -> Value {
        Value::from_seq_object(self.handle(value))
    }

    /// Creates a [`Value`] from a borrowed [`StructObject`].
    ///
    /// This is equivalent to `Value::from_struct_object(self.handle(value))`.
    pub fn struct_object_ref<'env, T: StructObject + 'static>(&'env self, value: &'env T) -> Value {
        Value::from_struct_object(self.handle(value))
    }
}

impl Drop for StackToken {
    fn drop(&mut self) {
        if self.unset {
            STACK_SCOPE_IS_VALID.with(|valid_ids| valid_ids.borrow_mut().remove(&self.id));
        }
    }
}

/// Creates a [`StackToken`].
#[macro_export]
macro_rules! stack_token {
    ($name:ident) => {
        #[allow(unsafe_code)]
        let $name = &unsafe { $crate::StackToken::__private_new() };
    };
}

#[test]
fn test_stack_handle() {
    let value = vec![1, 2, 3];

    let leaked_handle = {
        stack_token!(scope);
        let value_handle: StackHandle<Vec<i32>> = scope.handle(&value);
        assert_eq!(value_handle.with(|x| x.len()), 3);
        value_handle
    };

    assert_eq!(value.len(), 3);
    assert!(!StackHandle::is_valid(&leaked_handle));
}

#[test]
#[should_panic = "stack is gone"]
fn test_stack_handle_panic() {
    let value = vec![1, 2, 3];
    let leaked_handle = {
        stack_token!(scope);
        let value_handle: StackHandle<Vec<i32>> = scope.handle(&value);
        assert_eq!(value_handle.with(|x| x.len()), 3);
        value_handle
    };

    assert_eq!(leaked_handle.with(|x| x.len()), 3);
}
