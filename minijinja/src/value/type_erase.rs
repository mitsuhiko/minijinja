macro_rules! type_erase {
    ($v:vis trait $T:ident => $E:ident {
        $(fn $f:ident(&self $(, $p:ident : $t:ty $(,)?)*) $(-> $R:ty)?;)*

        $(
            impl $S:path {
                $(
                    fn $f_impl:ident[$f_vtable:ident](
                        &self $(, $p_impl:ident : $t_impl:ty $(,)?)*
                    ) $(-> $R_:ty)?;
                )*
            }
        )*
    }) => {
        #[doc = concat!("Type-erased version of [`", stringify!($T), "`]")]
        $v struct $E {
            ptr: *const (),
            vtable: *const (),
        }

        const _: () = {
            struct VTable {
                $($f: fn(*const (), $($p: $t),*) $(-> $R)?,)*
                $($($f_vtable: fn(*const (), $($p_impl: $t_impl),*) $(-> $R_)?,)*)*
                __type_id: fn() -> std::any::TypeId,
                __type_name: fn() -> &'static str,
                __drop: fn(*const ()),
            }

            #[inline(always)]
            fn vt(e: &$E) -> &VTable {
                unsafe { &*(e.vtable as *const VTable) }
            }

            impl $E {
                #[doc = concat!("Returns a new boxed, type-erased [`", stringify!($T), "`].")]
                $v fn new<T: $T + 'static>(v: std::sync::Arc<T>) -> Self {
                    let ptr = std::sync::Arc::into_raw(v) as *const T as *const ();
                    let vtable = &VTable {
                        $(
                            $f: |ptr, $($p),*| unsafe {
                                let arc: std::sync::Arc<T> = std::sync::Arc::from_raw(ptr as *const T);
                                let v = <T as $T>::$f(&arc, $($p),*);
                                std::mem::forget(arc);
                                v
                            },
                        )*
                        $($(
                            $f_vtable: |ptr, $($p_impl),*| unsafe {
                                let arc: std::sync::Arc<T> = std::sync::Arc::from_raw(ptr as *const T);
                                let v = <T as $S>::$f_impl(&*arc, $($p_impl),*);
                                std::mem::forget(arc);
                                v
                            },
                        )*)*
                        __type_id: || std::any::TypeId::of::<T>(),
                        __type_name: || std::any::type_name::<T>(),
                        __drop: |ptr| unsafe {
                            drop(std::sync::Arc::from_raw(ptr as *const T));
                        },
                    };

                    Self { ptr, vtable: vtable as *const VTable as *const () }
                }

                $(
                    #[doc = concat!("Calls [`", stringify!($T), "::", stringify!($f), "`] of the underlying boxed value.")]
                    $v fn $f(&self, $($p: $t),*) $(-> $R)? {
                        (vt(self).$f)(self.ptr, $($p),*)
                    }
                )*

                /// Returns the type name of the conrete underlying type.
                $v fn type_name(&self) -> &'static str {
                    (vt(self).__type_name)()
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
                    if (vt(self).__type_id)() == std::any::TypeId::of::<T>() {
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
                    if (vt(self).__type_id)() == std::any::TypeId::of::<T>() {
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
                    (vt(self).__drop)(self.ptr);
                }
            }

            impl<T: $T + 'static> From<Arc<T>> for $E {
                fn from(value: Arc<T>) -> Self {
                    $E::new(value)
                }
            }

            $(
                impl $S for $E {
                    $(
                        fn $f_impl(&self, $($p_impl: $t_impl),*) $(-> $R_)? {
                            (vt(self).$f_vtable)(self.ptr, $($p_impl),*)
                        }
                    )*
                }
            )*
        };
    };
}
