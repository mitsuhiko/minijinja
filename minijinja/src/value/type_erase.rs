macro_rules! type_erase {
    ($v:vis trait $T:ident $(: $($B:ident $(+)?)*)? => $E:ident($VT:ident) {
        $(fn $f:ident(&self $(, $p:ident : $t:ty $(,)?)*) $(-> $R:ty)?;)*

        $(
            impl $S:path {
                $(
                    fn $f_:ident[$f1:ident](
                        &self $(, $p_:ident : $t_:ty $(,)?)*
                    ) $(-> $R_:ty)?;
                )*
            }
        )*
    }) => {
        struct $VT {
            $($f: fn(&(), *const (), $($p : $t),*) $(-> $R)?,)*
            $($($f1: fn(*const (), $($p_ : $t_),*) $(-> $R_)?,)*)*
            type_id: fn() -> core::any::TypeId,
            type_name: fn() -> &'static str,
            drop: fn(*const ()),
        }

        /// Typed-erased version of
        #[doc = concat!("[`", stringify!($T), "`]")]
        $v struct $E {
            ptr: *const (),
            vtable: &'static $VT,
        }

        impl $E {
            /// Returns a new boxed, type-erased
            #[doc = concat!("[`", stringify!($T), "`].")]
            $v fn new<T: $T $($(+ $B)*)? + 'static>(v: std::sync::Arc<T>) -> Self {
                let ptr = std::sync::Arc::into_raw(v) as *const T as *const ();
                let vtable = &$VT {
                    $(
                        $f: |_, ptr, $($p),*| unsafe {
                            let arc: Arc<T> = std::sync::Arc::from_raw(ptr as *const T);
                            let v = <T as $T>::$f(&arc, $($p),*);
                            std::mem::forget(arc);
                            v
                        },
                    )*
                    $($(
                        $f1: |ptr, $($p_),*| unsafe {
                            let arc: Arc<T> = std::sync::Arc::from_raw(ptr as *const T);
                            let v = <T as $S>::$f_(&*arc, $($p_),*);
                            std::mem::forget(arc);
                            v
                        },
                    )*)*
                    type_id: || {
                        core::any::TypeId::of::<T>()
                    },
                    type_name: || {
                        core::any::type_name::<T>()
                    },
                    drop: |ptr| unsafe {
                        drop(std::sync::Arc::from_raw(ptr as *const T));
                    },
                };

                Self { ptr, vtable }
            }

            $(
                /// Calls
                #[doc = concat!("[`", stringify!($T), "::", stringify!($f), "`]")]
                /// on the underlying boxed value.
                $v fn $f(&self, $($p: $t),*) $(-> $R)? {
                    (self.vtable.$f)(&(), self.ptr, $($p),*)
                }
            )*

            /// Returns the type name of the conrete underlying type.
            $v fn type_name(&self) -> &'static str {
                (self.vtable.type_name)()
            }

            /// Downcast to `T` if the boxed value holds a `T`.
            ///
            /// This is basically the “reverse” of [`Value::from_object`].
            ///
            /// # Example
            ///
            /// ```
            /// # use minijinja::value::{Value, Object};
            /// use std::fmt;
            ///
            /// #[derive(Debug)]
            /// struct Thing {
            ///     id: usize,
            /// }
            ///
            /// impl Object for Thing {}
            ///
            /// let x_value = Value::from_object(Thing { id: 42 });
            /// let value_as_obj = x_value.as_object().unwrap();
            /// let thing = value_as_obj.downcast_ref::<Thing>().unwrap();
            /// assert_eq!(thing.id, 42);
            /// ```
            $v fn downcast_ref<T: 'static>(&self) -> Option<&T> {
                if (self.vtable.type_id)() == core::any::TypeId::of::<T>() {
                    unsafe {
                        return Some(&*(self.ptr as *const T));
                    }
                }

                None
            }

            /// Downcast to `T` if the boxed value holds a `T`.
            ///
            /// For details see [`downcast_ref`](Self::downcast_ref).
            $v fn downcast<T: 'static>(&self) -> Option<Arc<T>> {
                if (self.vtable.type_id)() == core::any::TypeId::of::<T>() {
                    unsafe {
                        let arc: Arc<T> = std::sync::Arc::from_raw(self.ptr as *const T);
                        let v = arc.clone();
                        std::mem::forget(arc);
                        return Some(v);
                    }
                }

                None
            }

            /// Checks if the boxed value is a `T`.
            ///
            /// For details see [`downcast_ref`](Self::downcast_ref).
            $v fn is<T: 'static>(&self) -> bool {
                self.downcast::<T>().is_some()
            }
        }

        impl Clone for $E {
            fn clone(&self) -> Self {
                unsafe {
                    std::sync::Arc::increment_strong_count(self.ptr);
                }

                Self {
                    ptr: self.ptr,
                    vtable: self.vtable,
                }
            }
        }

        impl Drop for $E {
            fn drop(&mut self) {
                (self.vtable.drop)(self.ptr);
            }
        }

        impl<T: $T $($(+ $B)*)? + 'static> From<Arc<T>> for $E {
            fn from(value: Arc<T>) -> Self {
                $E::new(value)
            }
        }

        $(
            impl $S for $E {
                $(
                    fn $f_(&self, $($p_: $t_),*) $(-> $R_)? {
                        (self.vtable.$f1)(self.ptr, $($p_),*)
                    }
                )*
            }
        )*

        $($(unsafe impl $B for $E { })*)?
    };
}
