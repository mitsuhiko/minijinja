macro_rules! type_erase {
    ($v:vis trait $T:ident $(: $($B:ident $(+)?)*)? => $E:ident($VT:ident) {
        $(fn $f:ident(&self $(, $p:ident : $t:ty),*) $(-> $R:ty)?;)*
    }) => {
        struct $VT {
            $($f: fn(*const (), $($p : $t),*) $(-> $R)?,)*
            type_id: fn() -> core::any::TypeId,
            drop: fn(*const ()),
        }

        /// Typed-erased version of
        #[doc = stringify!($T)]
        $v struct $E {
            ptr: *const (),
            vtable: &'static $VT,
        }

        impl $E {
            /// Returns a new type-erased `Any`.
            $v fn new<T: $T $($(+ $B)*)? + 'static>(v: std::sync::Arc<T>) -> Self {
                let ptr = std::sync::Arc::into_raw(v) as *const T as *const ();
                let vtable = &$VT {
                    $(
                        $f: |ptr, $($p),*| unsafe {
                            let arc = std::sync::Arc::from_raw(ptr as *const T);
                            let v = T::$f(&arc, $($p),*);
                            std::mem::forget(arc);
                            v
                        },
                    )*
                    type_id: || {
                        core::any::TypeId::of::<T>()
                    },
                    drop: |ptr| unsafe {
                        drop(std::sync::Arc::from_raw(ptr as *const T));
                    },
                };

                Self { ptr, vtable }
            }

            $(
                /// Calls the function with the same name in the inner struct.
                $v fn $f(&self, $($p: $t),*) $(-> $R)? {
                    (self.vtable.$f)(self.ptr, $($p),*)
                }
            )*

            /// Downcast to `T` if `self` holds a `T`.
            $v fn downcast<T: 'static>(&self) -> Option<&std::sync:: Arc<T>> {
                if (self.vtable.type_id)() == TypeId::of::<T>() {
                    unsafe {
                        let arc = std::sync::Arc::from_raw(self.ptr as *const T);
                        let reference: &'static std::sync::Arc<T> = std::mem::transmute(&arc);
                        std::mem::forget(arc);
                        return Some(reference);
                    }
                }

                None
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

        $($(unsafe impl $B for $E { })*)?
    };
}
