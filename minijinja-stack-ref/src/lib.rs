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
//! use minijinja_stack_ref::scope;
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
//! scope(|s| {
//!     println!("{}", tmpl.render(context! {
//!         // note how it's possible to create a handle to the struct
//!         // object by reference, and how it can be wrapped in a value.
//!         config => Value::from_struct_object(s.handle(&state)),
//!     }).unwrap());
//! });
//! ```
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};

use minijinja::value::{Object, ObjectKind, SeqObject, StructObject, Value};
use minijinja::{Error, State};

static STACK_SCOPE_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static STACK_SCOPE_IS_VALID: RefCell<HashSet<u64>> = RefCell::default();
}

/// A handle to an enclosed value.
///
/// For as long as the [`StackScope`] is still valid this derefs
/// automatically into the enclosed value.  Doing so after the
/// scope is gone, this will panic on all operations.
///
/// To check if a handle is still valid [`is_valid`](Self::is_valid)
/// can be used.
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
}

impl<T> Deref for StackHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        assert!(StackHandle::is_valid(self), "stack is gone");
        unsafe { &*self.ptr as &T }
    }
}

impl<T: SeqObject + Send + Sync + 'static> SeqObject for StackHandle<T> {
    fn get_item(&self, idx: usize) -> Option<Value> {
        <T as SeqObject>::get_item(self, idx)
    }

    fn item_count(&self) -> usize {
        <T as SeqObject>::item_count(self)
    }
}

impl<T: StructObject + Send + Sync + 'static> StructObject for StackHandle<T> {
    fn get_field(&self, idx: &str) -> Option<Value> {
        <T as StructObject>::get_field(self, idx)
    }

    fn fields(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        <T as StructObject>::fields(self)
    }

    fn field_count(&self) -> usize {
        <T as StructObject>::field_count(self)
    }
}

impl<T: Object> fmt::Debug for StackHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self as &T, f)
    }
}

impl<T: Object> fmt::Display for StackHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self as &T, f)
    }
}

impl<T: Object> Object for StackHandle<T> {
    fn kind(&self) -> ObjectKind<'_> {
        <T as Object>::kind(self)
    }

    fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        <T as Object>::call_method(self, state, name, args)
    }

    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        <T as Object>::call(self, state, args)
    }
}

/// A scope to enclose references to stack values.
///
/// See [`scope`] for details.
pub struct StackScope<'scope, 'env: 'scope> {
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
    id: u64,
}

impl<'scope, 'env: 'scope> StackScope<'scope, 'env> {
    /// Creates a [`StackHandle`] to a value with at least the scope's lifetime.
    pub fn handle<T: 'env>(&self, value: &'env T) -> StackHandle<T> {
        StackHandle {
            ptr: value as *const T,
            id: self.id,
        }
    }
}

/// Run a function in the context of a stack scope.
pub fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&'scope StackScope<'scope, 'env>) -> T,
{
    let scope = StackScope {
        env: PhantomData,
        scope: PhantomData,
        id: STACK_SCOPE_COUNTER.fetch_add(1, Ordering::SeqCst),
    };

    STACK_SCOPE_IS_VALID.with(|valid_ids| {
        let marked_scope_valid = valid_ids.borrow_mut().insert(scope.id);
        let rv = catch_unwind(AssertUnwindSafe(|| f(&scope)));
        if marked_scope_valid {
            valid_ids.borrow_mut().remove(&scope.id);
        }
        match rv {
            Err(e) => resume_unwind(e),
            Ok(result) => result,
        }
    })
}

#[test]
fn test_stack_handle() {
    let value = vec![1, 2, 3];
    let leaked_handle = scope(|scope| {
        let value_handle: StackHandle<Vec<i32>> = scope.handle(&value);
        assert_eq!(value_handle.len(), 3);
        value_handle
    });

    assert_eq!(value.len(), 3);
    assert!(!StackHandle::is_valid(&leaked_handle));
}

#[test]
#[should_panic = "stack is gone"]
fn test_stack_handle_panic() {
    let value = vec![1, 2, 3];
    let leaked_handle = scope(|scope| {
        let value_handle = scope.handle(&value);
        assert_eq!(value_handle.len(), 3);
        value_handle
    });

    assert_eq!(leaked_handle.len(), 3);
}
