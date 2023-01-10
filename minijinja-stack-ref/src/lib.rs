//! An extension package to MiniJinja that allows stack borrows.
//!
//! **This is an experimental package. There might be soundness issues and there
//! might be problems with the API. Please give feedback.**
//!
//! # Intro
//!
//! When implementing dynamic objects for MiniJinja lifetimes a common hurdle can
//! be lifetimes.  That's because MiniJinja requires that all values passed to the
//! template are owned by the runtime engine.  Thus it becomes impossible to carry
//! non static lifetimes into the template.
//!
//! This crate provides a solution to this issue by moving lifetime checks to the
//! runtime for MiniJinja objects.  One first needs to create a [`Scope`] with
//! the [`scope`] function.  It invokes a callback to which a scope is passed
//! which in turn then provides functionality to create
//! [`Value`](minijinja::value::Value)s to those borrowed values such as the
//! [`object_ref`](crate::Scope::object_ref) method.
//!
//! # Example
//!
//! This example demonstrates how to pass borrowed information into a template:
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
//!     "app version: {{ state.version }}\nitems: {{ items }}"
//! )
//! .unwrap();
//!
//! let state = State {
//!     version: env!("CARGO_PKG_VERSION"),
//! };
//! let items = [1u32, 2, 3, 4];
//!
//! let rv = scope(|scope| {
//!     let tmpl = env.get_template("info").unwrap();
//!     tmpl.render(context! {
//!         state => scope.struct_object_ref(&state),
//!         items => scope.seq_object_ref(&items[..]),
//!     }).unwrap()
//! });
//! println!("{}", rv);
//! ```
//!
//! # Reborrowing
//!
//! If an object holds other complex values it can be interesting to again
//! return a reference to a member rather.  In that case it becomes necessary
//! again to get access to the [`Scope`].  This can be accomplished with the
//! [`reborrow`] functionality which.  It lets you return references to `&self`
//! from within an referenced object:
//!
//! ```
//! use minijinja::value::{StructObject, Value};
//! use minijinja::{context, Environment};
//! use minijinja_stack_ref::{reborrow, scope};
//!
//! struct Config {
//!     version: &'static str,
//! }
//!
//! struct State {
//!     config: Config,
//! }
//!
//! impl StructObject for Config {
//!     fn get_field(&self, field: &str) -> Option<Value> {
//!         match field {
//!             "version" => Some(Value::from(self.version)),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! impl StructObject for State {
//!     fn get_field(&self, field: &str) -> Option<Value> {
//!         match field {
//!             // return a reference to the inner config through reborrowing
//!             "config" => Some(reborrow(self, |slf, scope| {
//!                 scope.struct_object_ref(&slf.config)
//!             })),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! let mut env = Environment::new();
//! env.add_template(
//!     "info",
//!     "app version: {{ state.config.version }}"
//! )
//! .unwrap();
//!
//! let state = State {
//!     config: Config {
//!         version: env!("CARGO_PKG_VERSION"),
//!     }
//! };
//!
//! let rv = scope(|scope| {
//!     let tmpl = env.get_template("info").unwrap();
//!     tmpl.render(context! {
//!         state => scope.struct_object_ref(&state),
//!     }).unwrap()
//! });
//! println!("{}", rv);
//! ```
use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::fmt;
use std::marker::PhantomData;
use std::mem::transmute;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::Arc;

use minijinja::value::{Object, ObjectKind, SeqObject, StructObject, Value};
use minijinja::{Error, State};

static STACK_SCOPE_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static STACK_SCOPE_IS_VALID: RefCell<HashSet<u64>> = RefCell::default();
    static CURRENT_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
}

/// A handle to an enclosed value.
///
/// For as long as the [`Scope`] is still valid access to the
/// reference can be temporarily fetched via the [`with`](Self::with)
/// method.  Doing so after the scope is gone, this will panic on all
/// operations.
///
/// To check if a handle is still valid [`is_valid`](Self::is_valid)
/// can be used.
///
/// A stack handle implements the underlying object protocols from
/// MiniJinja.
pub struct StackHandle<T: ?Sized> {
    ptr: *const T,
    id: u64,
}

unsafe impl<T: Send + ?Sized> Send for StackHandle<T> {}
unsafe impl<T: Sync + ?Sized> Sync for StackHandle<T> {}

struct ResetHandleOnDrop(*mut c_void);

impl Drop for ResetHandleOnDrop {
    fn drop(&mut self) {
        CURRENT_HANDLE.with(|handle| handle.store(self.0, Ordering::SeqCst));
    }
}

/// Reborrows a reference to a dynamic object with the scope's lifetime.
///
/// Within the trait methods of [`Object`], [`StructObject`] or [`SeqObject`] of a
/// value that is currently referenced by a [`StackHandle`], this utility can be
/// used to reborrow `&self` with the lifetime of the scope.
///
/// This lets code return a [`Value`] that borrows into a field of `&self`.
///
/// ```
/// use minijinja::value::{Value, StructObject};
/// use minijinja_stack_ref::{reborrow, scope};
///
/// struct MyObject {
///     values: Vec<u32>,
/// }
///
/// impl StructObject for MyObject {
///     fn get_field(&self, field: &str) -> Option<Value> {
///         match field {
///             "values" => Some(reborrow(self, |slf, scope| {
///                 scope.seq_object_ref(&slf.values[..])
///             })),
///             _ => None
///         }
///     }
/// }
///
/// let obj = MyObject { values: (0..100).collect() };
/// scope(|scope| {
///     let value = scope.struct_object_ref(&obj);
///     // do something with value
/// #   let _ = value;
/// })
/// ```
///
/// # Panics
///
/// This function panics if the passed object is not currently interacted with
/// or not created via the [`Scope`].  In other words this function can only be
/// used within object methods of [`Object`], [`SeqObject`] or [`StructObject`]
/// of an object that has been put into a [`Value`] via a [`Scope`].
///
/// To check if reborrowing is possible, [`can_reborrow`] can be used instead.
pub fn reborrow<T: ?Sized, R>(obj: &T, f: for<'a> fn(&'a T, &'a Scope) -> R) -> R {
    CURRENT_HANDLE.with(|handle_ptr| {
        let handle = match unsafe {
            (handle_ptr.load(Ordering::SeqCst) as *const StackHandle<T>).as_ref()
        } {
            Some(handle) => handle,
            None => {
                panic!(
                    "cannot reborrow &{} because there is no handle on the stack",
                    std::any::type_name::<T>()
                );
            }
        };

        if handle.ptr != obj as *const T {
            panic!(
                "cannot reborrow &{} as it's not held in an active stack handle ({:?} != {:?})",
                std::any::type_name::<T>(),
                handle.ptr,
                obj as *const T,
            );
        }

        assert!(
            StackHandle::is_valid(handle),
            "cannot reborrow &{} because stack is gone",
            std::any::type_name::<T>()
        );

        let scope = Scope {
            id: handle.id,
            unset: false,
            _marker: PhantomData,
        };
        f(unsafe { &*handle.ptr as &T }, &scope)
    })
}

/// Returns `true` if reborrowing is possible.
///
/// This can be used to make an object conditionally reborrow.  If this method returns
/// `true`, then [`reborrow`] will not panic.
///
/// ```
/// use minijinja::value::{Value, StructObject};
/// use minijinja_stack_ref::{reborrow, can_reborrow, scope};
///
/// struct MyObject {
///     values: Vec<u32>,
/// }
///
/// impl StructObject for MyObject {
///     fn get_field(&self, field: &str) -> Option<Value> {
///         match field {
///             "values" => if can_reborrow(self) {
///                 Some(reborrow(self, |slf, scope| {
///                     scope.seq_object_ref(&slf.values[..])
///                 }))
///             } else {
///                 Some(Value::from_serializable(&self.values))
///             },
///             _ => None
///         }
///     }
/// }
/// ```
pub fn can_reborrow<T: ?Sized>(obj: &T) -> bool {
    CURRENT_HANDLE.with(|handle_ptr| {
        let handle = match unsafe {
            (handle_ptr.load(Ordering::SeqCst) as *const StackHandle<T>).as_ref()
        } {
            Some(handle) => handle,
            None => return false,
        };

        if handle.ptr != obj as *const T {
            return false;
        }

        StackHandle::is_valid(handle)
    })
}

impl<T: ?Sized> StackHandle<T> {
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
        let _reset = ResetHandleOnDrop(
            CURRENT_HANDLE
                .with(|handle| handle.swap(self as *const _ as *mut c_void, Ordering::SeqCst)),
        );
        f(unsafe { &*self.ptr as &T })
    }
}

impl<T: SeqObject + Send + Sync + 'static + ?Sized> SeqObject for StackHandle<T> {
    fn get_item(&self, idx: usize) -> Option<Value> {
        self.with(|val| val.get_item(idx))
    }

    fn item_count(&self) -> usize {
        self.with(|val| val.item_count())
    }
}

impl<T: StructObject + Send + Sync + 'static + ?Sized> StructObject for StackHandle<T> {
    fn get_field(&self, idx: &str) -> Option<Value> {
        self.with(|val| val.get_field(idx))
    }

    fn static_fields(&self) -> Option<&'static [&'static str]> {
        self.with(|val| val.static_fields())
    }

    fn fields(&self) -> Vec<Arc<String>> {
        self.with(|val| val.fields())
    }

    fn field_count(&self) -> usize {
        self.with(|val| val.field_count())
    }
}

impl<T: Object + ?Sized> fmt::Debug for StackHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|val| fmt::Debug::fmt(val, f))
    }
}

impl<T: Object + ?Sized> fmt::Display for StackHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|val| fmt::Display::fmt(val, f))
    }
}

impl<T: Object + ?Sized> Object for StackHandle<T> {
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
struct StackHandleProxy<T: Object + ?Sized>(StackHandle<T>);

macro_rules! unwrap_kind {
    ($val:expr, $pat:path) => {
        if let $pat(rv) = $val.kind() {
            rv
        } else {
            unreachable!("object changed shape")
        }
    };
}

impl<T: Object + ?Sized> SeqObject for StackHandleProxy<T> {
    fn get_item(&self, idx: usize) -> Option<Value> {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Seq).get_item(idx))
    }

    fn item_count(&self) -> usize {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Seq).item_count())
    }
}

impl<T: Object + ?Sized> StructObject for StackHandleProxy<T> {
    fn get_field(&self, name: &str) -> Option<Value> {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Struct).get_field(name))
    }

    fn fields(&self) -> Vec<Arc<String>> {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Struct).fields())
    }

    fn field_count(&self) -> usize {
        self.0
            .with(|val| unwrap_kind!(val, ObjectKind::Struct).field_count())
    }
}

/// Captures the calling scope.
///
/// To create a new scope, [`scope`] can be used.  To get the current active scope the
/// [`reborrow`] functionality is available.
pub struct Scope {
    id: u64,
    unset: bool,
    _marker: PhantomData<*const ()>,
}

impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scope").field("id", &self.id).finish()
    }
}

impl Scope {
    fn new() -> Scope {
        let id = STACK_SCOPE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let unset = STACK_SCOPE_IS_VALID.with(|valid_ids| valid_ids.borrow_mut().insert(id));
        Scope {
            id,
            unset,
            _marker: PhantomData,
        }
    }

    /// Creates a [`StackHandle`] to a value with at least the scope's lifetime.
    pub fn handle<'env, T: 'env + ?Sized>(&'env self, value: &'env T) -> StackHandle<T> {
        StackHandle {
            ptr: value as *const T,
            id: self.id,
        }
    }

    /// Creates a [`Value`] from a borrowed [`Object`].
    ///
    /// This is equivalent to `Value::from_object(self.handle(value))`.
    pub fn object_ref<'env, T: Object + ?Sized>(&'env self, value: &'env T) -> Value {
        Value::from_object(self.handle(value))
    }

    /// Creates a [`Value`] from a borrowed [`SeqObject`].
    ///
    /// This is equivalent to `Value::from_seq_object(self.handle(value))`.
    pub fn seq_object_ref<'env, T: SeqObject + 'static + ?Sized>(
        &'env self,
        value: &'env T,
    ) -> Value {
        Value::from_seq_object(self.handle(value))
    }

    /// Creates a [`Value`] from a borrowed [`StructObject`].
    ///
    /// This is equivalent to `Value::from_struct_object(self.handle(value))`.
    pub fn struct_object_ref<'env, T: StructObject + 'static + ?Sized>(
        &'env self,
        value: &'env T,
    ) -> Value {
        Value::from_struct_object(self.handle(value))
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if self.unset {
            STACK_SCOPE_IS_VALID.with(|valid_ids| valid_ids.borrow_mut().remove(&self.id));
        }
    }
}

/// Invokes a function with a reference to the stack scope so values can be borrowed.
///
/// ```
/// # use minijinja_stack_ref::scope;
/// use minijinja::render;
///
/// let items = [1u32, 2, 3, 4];
/// let rv = scope(|scope| {
///     render!("items: {{ items }}", items => scope.seq_object_ref(&items[..]))
/// });
/// assert_eq!(rv, "items: [1, 2, 3, 4]");
/// ```
pub fn scope<R, F: FnOnce(&Scope) -> R>(f: F) -> R {
    f(&Scope::new())
}

#[test]
fn test_stack_handle() {
    let value = vec![1, 2, 3];

    let leaked_handle = {
        scope(|scope| {
            let value_handle: StackHandle<Vec<i32>> = scope.handle(&value);
            assert_eq!(value_handle.with(|x| x.len()), 3);
            value_handle
        })
    };

    assert_eq!(value.len(), 3);
    assert!(!StackHandle::is_valid(&leaked_handle));
}

#[test]
#[should_panic = "stack is gone"]
fn test_stack_handle_panic() {
    let value = vec![1, 2, 3];
    let leaked_handle = {
        scope(|scope| {
            let value_handle: StackHandle<Vec<i32>> = scope.handle(&value);
            assert_eq!(value_handle.with(|x| x.len()), 3);
            value_handle
        })
    };

    assert_eq!(leaked_handle.with(|x| x.len()), 3);
}
