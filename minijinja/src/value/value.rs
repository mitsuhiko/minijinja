use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use crate::functions;
use crate::error::{Error, ErrorKind};
use crate::vm::State;

use crate::value::map::{ValueMap, OwnedValueMap};
use crate::value::object::{SimpleSeqObject, SimpleStructObject};
use crate::value::ops;
use crate::value::intern;

use crate::value::Value;
use crate::value::argtypes::{FunctionArgs, FunctionResult};
use crate::value::object::{Object, ObjectKind, SeqObject, StructObject};

#[derive(Clone)]
pub enum ValueBuf {
    Undefined,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    None,
    Invalid(Arc<str>),
    U128(Packed<u128>),
    I128(Packed<i128>),
    String(Arc<str>, StringType),
    Bytes(Arc<[u8]>),
    Seq(Arc<dyn SeqObject>),
    Map(Arc<dyn StructObject>, MapType),
    // Map(Arc<OwnedValueMap>, MapType),
    Dynamic(Arc<dyn Object>),
}

#[derive(Debug, Clone)]
pub enum ValueBufX<'a> {
    None,
    Undefined,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    U128(Packed<u128>),
    I128(Packed<i128>),
    Invalid(ArcCow<'a, str>),
    String(ArcCow<'a, str>, StringType),
    Bytes(ArcCow<'a, [u8]>),
    Seq(ArcCow<'a, dyn SeqObject + 'a>),
    Map(ArcCow<'a, ValueMap<'static>>, MapType),
    Dynamic(ArcCow<'a, dyn Object>),
}

impl<'a> fmt::Debug for dyn SeqObject + 'a {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a> std::hash::Hash for dyn SeqObject + 'a {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.iter().for_each(|v| v.hash(state));
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for Packed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.0;
        f.debug_tuple("Packed").field(&v).finish()
    }
}

pub enum ArcCow<'a, T: ?Sized + 'a> {
    Borrowed(&'a T),
    Owned(Arc<T>),
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for ArcCow<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Borrowed(arg0) => f.debug_tuple("Borrowed").field(arg0).finish(),
            Self::Owned(arg0) => f.debug_tuple("Owned").field(arg0).finish(),
        }
    }
}

impl<T: ?Sized> Clone for ArcCow<'_, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Borrowed(v) => Self::Borrowed(v),
            Self::Owned(v) => Self::Owned(v.clone()),
        }
    }
}

impl<'a, T: ?Sized + 'a> ArcCow<'a, T> {
    pub fn new(value: &'a T) -> ArcCow<'a, T> {
        ArcCow::Borrowed(value)
    }
}

impl<'a, V: ?Sized, T: Into<Arc<V>> + ?Sized> From<T> for ArcCow<'a, V> {
    fn from(value: T) -> Self {
        ArcCow::Owned(value.into())
    }
}

#[derive(Clone)]
pub enum ValueCow<'a> {
    Owned(ValueBuf),
    Borrowed(ValueRef<'a>)
}

#[derive(Clone)]
pub enum ValueRef<'a> {
    Undefined,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    None,
    Invalid(&'a str),
    U128(Packed<u128>),
    I128(Packed<i128>),
    String(&'a str, StringType),
    Bytes(&'a [u8]),
    Seq(&'a [ValueCow<'a>]),
    Map(&'a OwnedValueMap, MapType),
    Dynamic(&'a dyn Object),
}

/// Wraps an internal copyable value but marks it as packed.
///
/// This is used for `i128`/`u128` in the value repr to avoid
/// the excessive 16 byte alignment.
#[derive(Copy)]
#[repr(packed)]
pub struct Packed<T: Copy>(pub T);

/// The type of map
#[derive(Copy, Clone, Debug)]
pub enum MapType {
    /// A regular map
    Normal,
    /// A map representing keyword arguments
    Kwargs,
}

/// Type type of string
#[derive(Copy, Clone, Debug)]
pub enum StringType {
    Normal,
    Safe,
}

/// Describes the kind of value.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ValueKind {
    /// The value is undefined
    Undefined,
    /// The value is the none singleton ([`()`])
    None,
    /// The value is a [`bool`]
    Bool,
    /// The value is a number of a supported type.
    Number,
    /// The value is a string.
    String,
    /// The value is a byte array.
    Bytes,
    /// The value is an array of other values.
    Seq,
    /// The value is a key/value mapping.
    Map,
}

#[allow(clippy::len_without_is_empty)]
impl Value {
    /// The undefined value.
    ///
    /// This constant exists because the undefined type does not exist in Rust
    /// and this is the only way to construct it.
    pub const UNDEFINED: Value = Value(ValueBuf::Undefined);

    /// Creates a value from a safe string.
    ///
    /// A safe string is one that will bypass auto escaping.  For instance if you
    /// want to have the template engine render some HTML without the user having to
    /// supply the `|safe` filter, you can use a value of this type instead.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let val = Value::from_safe_string("<em>note</em>".into());
    /// ```
    pub fn from_safe_string(value: String) -> Value {
        ValueBuf::String(Arc::from(value), StringType::Safe).into()
    }

    /// Creates a value from a dynamic object.
    ///
    /// For more information see [`Object`].
    ///
    /// ```rust
    /// # use minijinja::value::{Value, Object};
    /// use std::fmt;
    ///
    /// #[derive(Debug)]
    /// struct Thing {
    ///     id: usize,
    /// }
    ///
    /// impl fmt::Display for Thing {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt::Debug::fmt(self, f)
    ///     }
    /// }
    ///
    /// impl Object for Thing {}
    ///
    /// let val = Value::from_object(Thing { id: 42 });
    /// ```
    ///
    /// Objects are internally reference counted.  If you want to hold on to the
    /// `Arc` you can directly create the value from an arc'ed object:
    ///
    /// ```rust
    /// # use minijinja::value::{Value, Object};
    /// # #[derive(Debug)]
    /// # struct Thing { id: usize };
    /// # impl std::fmt::Display for Thing {
    /// #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    /// #         todo!();
    /// #     }
    /// # }
    /// # impl Object for Thing {}
    /// use std::sync::Arc;
    /// let val = Value::from(Arc::new(Thing { id: 42 }));
    /// ```
    pub fn from_object<T: Object>(value: T) -> Value {
        Value::from(Arc::new(value) as Arc<dyn Object>)
    }

    /// Creates a value from an owned [`SeqObject`].
    ///
    /// This is a simplified API for creating dynamic sequences
    /// without having to implement the entire [`Object`] protocol.
    ///
    /// **Note:** objects created this way cannot be downcasted via
    /// [`downcast_object_ref`](Self::downcast_object_ref).
    pub fn from_seq_object<T: SeqObject + 'static>(value: T) -> Value {
        Value::from_object(SimpleSeqObject(value))
    }

    /// Creates a value from an owned [`StructObject`].
    ///
    /// This is a simplified API for creating dynamic structs
    /// without having to implement the entire [`Object`] protocol.
    ///
    /// **Note:** objects created this way cannot be downcasted via
    /// [`downcast_object_ref`](Self::downcast_object_ref).
    pub fn from_struct_object<T: StructObject + 'static>(value: T) -> Value {
        Value::from_object(SimpleStructObject(value))
    }

    /// Creates a callable value from a function.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let pow = Value::from_function(|a: u32| a * a);
    /// ```
    pub fn from_function<F, Rv, Args>(f: F) -> Value
    where
        // the crazy bounds here exist to enable borrowing in closures
        F: functions::Function<Rv, Args>
            + for<'a> functions::Function<Rv, <Args as FunctionArgs<'a>>::Output>,
        Rv: FunctionResult,
        Args: for<'a> FunctionArgs<'a>,
    {
        functions::BoxedFunction::new(f).to_value()
    }

    /// Returns the kind of the value.
    ///
    /// This can be used to determine what's in the value before trying to
    /// perform operations on it.
    pub fn kind(&self) -> ValueKind {
        match self.0 {
            ValueBuf::Undefined => ValueKind::Undefined,
            ValueBuf::Bool(_) => ValueKind::Bool,
            ValueBuf::U64(_) | ValueBuf::I64(_) | ValueBuf::F64(_) => ValueKind::Number,
            ValueBuf::None => ValueKind::None,
            ValueBuf::I128(_) => ValueKind::Number,
            ValueBuf::String(..) => ValueKind::String,
            ValueBuf::Bytes(_) => ValueKind::Bytes,
            ValueBuf::U128(_) => ValueKind::Number,
            ValueBuf::Seq(_) => ValueKind::Seq,
            ValueBuf::Map(..) => ValueKind::Map,
            // XXX: invalid values report themselves as maps which is a lie
            ValueBuf::Invalid(_) => ValueKind::Map,
            ValueBuf::Dynamic(ref dy) => match dy.kind() {
                // XXX: basic objects should probably not report as map
                ObjectKind::Plain => ValueKind::Map,
                ObjectKind::Seq(_) => ValueKind::Seq,
                ObjectKind::Struct(_) => ValueKind::Map,
            },
        }
    }

    /// Returns `true` if the value is a number.
    ///
    /// To convert a value into a primitive number, use [`TryFrom`] or [`TryInto`].
    pub fn is_number(&self) -> bool {
        matches!(
            self.0,
            ValueBuf::U64(_)
                | ValueBuf::I64(_)
                | ValueBuf::F64(_)
                | ValueBuf::I128(_)
                | ValueBuf::U128(_)
        )
    }

    /// Returns `true` if the map represents keyword arguments.
    pub fn is_kwargs(&self) -> bool {
        matches!(self.0, ValueBuf::Map(_, MapType::Kwargs))
    }

    /// Is this value true?
    pub fn is_true(&self) -> bool {
        match self.0 {
            ValueBuf::Bool(val) => val,
            ValueBuf::U64(x) => x != 0,
            ValueBuf::U128(x) => x.0 != 0,
            ValueBuf::I64(x) => x != 0,
            ValueBuf::I128(x) => x.0 != 0,
            ValueBuf::F64(x) => x != 0.0,
            ValueBuf::String(ref x, _) => !x.is_empty(),
            ValueBuf::Bytes(ref x) => !x.is_empty(),
            ValueBuf::None | ValueBuf::Undefined | ValueBuf::Invalid(_) => false,
            ValueBuf::Seq(ref x) => x.item_count() != 0,
            ValueBuf::Map(ref x, _) => x.field_count() != 0,
            ValueBuf::Dynamic(ref x) => match x.kind() {
                ObjectKind::Plain => true,
                ObjectKind::Seq(s) => s.item_count() != 0,
                ObjectKind::Struct(s) => s.field_count() != 0,
            },
        }
    }

    /// Returns `true` if this value is safe.
    pub fn is_safe(&self) -> bool {
        matches!(&self.0, ValueBuf::String(_, StringType::Safe))
    }

    /// Returns `true` if this value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(&self.0, ValueBuf::Undefined)
    }

    /// Returns `true` if this value is none.
    pub fn is_none(&self) -> bool {
        matches!(&self.0, ValueBuf::None)
    }

    /// If the value is a string, return it.
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            ValueBuf::String(ref s, _) => Some(s as &str),
            _ => None,
        }
    }

    /// If this is an i64 return it
    pub fn as_i64(&self) -> Option<i64> {
        i64::try_from(self.clone()).ok()
    }

    /// Returns the bytes of this value if they exist.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0 {
            ValueBuf::String(ref s, _) => Some(s.as_bytes()),
            ValueBuf::Bytes(ref b) => Some(&b[..]),
            _ => None,
        }
    }

    /// If the value is an object, it's returned as [`Object`].
    pub fn as_object(&self) -> Option<&dyn Object> {
        match self.0 {
            ValueBuf::Dynamic(ref dy) => Some(&**dy as &dyn Object),
            _ => None,
        }
    }

    /// If the value is a sequence it's returned as [`SeqObject`].
    pub fn as_seq(&self) -> Option<&dyn SeqObject> {
        match self.0 {
            ValueBuf::Seq(ref v) => return Some(&*v as &dyn SeqObject),
            ValueBuf::Dynamic(ref dy) => {
                if let ObjectKind::Seq(seq) = dy.kind() {
                    return Some(seq);
                }
            }
            _ => {}
        }
        None
    }

    /// If the value is a struct, return it as [`StructObject`].
    pub fn as_struct(&self) -> Option<&dyn StructObject> {
        if let ValueBuf::Dynamic(ref dy) = self.0 {
            if let ObjectKind::Struct(s) = dy.kind() {
                return Some(s);
            }
        }
        None
    }

    /// Returns the length of the contained value.
    ///
    /// Values without a length will return `None`.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let seq = Value::from(vec![1, 2, 3, 4]);
    /// assert_eq!(seq.len(), Some(4));
    /// ```
    pub fn len(&self) -> Option<usize> {
        match self.0 {
            ValueBuf::String(ref s, _) => Some(s.chars().count()),
            ValueBuf::Map(ref items, _) => Some(items.field_count()),
            ValueBuf::Seq(ref items) => Some(items.item_count()),
            ValueBuf::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Plain => None,
                ObjectKind::Seq(s) => Some(s.item_count()),
                ObjectKind::Struct(s) => Some(s.field_count()),
            },
            _ => None,
        }
    }

    /// Looks up an attribute by attribute name.
    ///
    /// This this returns [`UNDEFINED`](Self::UNDEFINED) when an invalid key is
    /// resolved.  An error is returned when if the value does not contain an object
    /// that has attributes.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// # fn test() -> Result<(), minijinja::Error> {
    /// let ctx = minijinja::context! {
    ///     foo => "Foo"
    /// };
    /// let value = ctx.get_attr("foo")?;
    /// assert_eq!(value.to_string(), "Foo");
    /// # Ok(()) }
    /// ```
    pub fn get_attr(&self, key: &str) -> Result<Value, Error> {
        Ok(match self.0 {
            ValueBuf::Undefined => return Err(Error::from(ErrorKind::UndefinedError)),
            ValueBuf::Map(ref items, _) => items.get_field(&Value::from(key)),
            ValueBuf::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Struct(s) => s.get_field(&Value::from(key)),
                ObjectKind::Plain | ObjectKind::Seq(_) => None,
            },
            _ => None,
        }
        .unwrap_or(Value::UNDEFINED))
    }

    /// Alternative lookup strategy without error handling exclusively for context
    /// resolution.
    ///
    /// The main difference is that the return value will be `None` if the value is
    /// unable to look up the key rather than returning `Undefined` and errors will
    /// also not be created.
    pub(crate) fn get_attr_fast(&self, key: &str) -> Option<Value> {
        match self.0 {
            ValueBuf::Map(ref items, _) => items.get_field(&key.into()),
            ValueBuf::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Struct(s) => s.get_field(&key.into()),
                ObjectKind::Plain | ObjectKind::Seq(_) => None,
            },
            _ => None,
        }
    }

    /// Looks up an index of the value.
    ///
    /// This is a shortcut for [`get_item`](Self::get_item).
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let seq = Value::from(vec![0u32, 1, 2]);
    /// let value = seq.get_item_by_index(1).unwrap();
    /// assert_eq!(value.try_into().ok(), Some(1));
    /// ```
    pub fn get_item_by_index(&self, idx: usize) -> Result<Value, Error> {
        self.get_item(&Value(ValueBuf::U64(idx as _)))
    }

    /// Looks up an item (or attribute) by key.
    ///
    /// This is similar to [`get_attr`](Self::get_attr) but instead of using
    /// a string key this can be any key.  For instance this can be used to
    /// index into sequences.  Like [`get_attr`](Self::get_attr) this returns
    /// [`UNDEFINED`](Self::UNDEFINED) when an invalid key is looked up.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// let ctx = minijinja::context! {
    ///     foo => "Foo",
    /// };
    /// let value = ctx.get_item(&Value::from("foo")).unwrap();
    /// assert_eq!(value.to_string(), "Foo");
    /// ```
    pub fn get_item(&self, key: &Value) -> Result<Value, Error> {
        if let ValueBuf::Undefined = self.0 {
            Err(Error::from(ErrorKind::UndefinedError))
        } else {
            Ok(self.get_item_opt(key).unwrap_or(Value::UNDEFINED))
        }
    }

    /// Iterates over the value.
    ///
    /// Depending on the [`kind`](Self::kind) of the value the iterator
    /// has a different behavior.
    ///
    /// * [`ValueKind::Map`]: the iterator yields the keys of the map.
    /// * [`ValueKind::Seq`]: the iterator yields the items in the sequence.
    /// * [`ValueKind::None`] / [`ValueKind::Undefined`]: the iterator is empty.
    ///
    /// ```
    /// # use minijinja::value::Value;
    /// # fn test() -> Result<(), minijinja::Error> {
    /// let value = Value::from({
    ///     let mut m = std::collections::BTreeMap::new();
    ///     m.insert("foo", 42);
    ///     m.insert("bar", 23);
    ///     m
    /// });
    /// for key in value.try_iter()? {
    ///     let value = value.get_item(&key)?;
    ///     println!("{} = {}", key, value);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn try_iter(&self) -> Result<ValueIter<'_>, Error> {
        self.try_iter_owned().map(|inner| ValueIter {
            _marker: PhantomData,
            inner,
        })
    }

    /// Returns some reference to the boxed object if it is of type `T`, or None if it isn’t.
    ///
    /// This is basically the "reverse" of [`from_object`](Self::from_object).  It's also
    /// a shortcut for [`downcast_ref`](trait.Object.html#method.downcast_ref)
    /// on the return value of [`as_object`](Self::as_object).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use minijinja::value::{Value, Object};
    /// use std::fmt;
    ///
    /// #[derive(Debug)]
    /// struct Thing {
    ///     id: usize,
    /// }
    ///
    /// impl fmt::Display for Thing {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt::Debug::fmt(self, f)
    ///     }
    /// }
    ///
    /// impl Object for Thing {}
    ///
    /// let x_value = Value::from_object(Thing { id: 42 });
    /// let thing = x_value.downcast_object_ref::<Thing>().unwrap();
    /// assert_eq!(thing.id, 42);
    /// ```
    pub fn downcast_object_ref<T: Object>(&self) -> Option<&T> {
        self.as_object().and_then(|x| x.downcast_ref())
    }

    pub(crate) fn get_item_opt(&self, key: &Value) -> Option<Value> {
        let seq = match self.0 {
            ValueBuf::Map(ref items, _) => return items.get_field(key),
            ValueBuf::Seq(ref items) => &*items as &dyn SeqObject,
            ValueBuf::Dynamic(ref dy) => match dy.kind() {
                ObjectKind::Plain => return None,
                ObjectKind::Seq(s) => s,
                ObjectKind::Struct(s) => {
                    return if let Some(key) = key.as_str() {
                        s.get_field(&key.into())
                    } else {
                        None
                    };
                }
            },
            ValueBuf::String(ref s, _) => {
                if let Some(idx) = key.as_i64() {
                    let idx = some!(isize::try_from(idx).ok());
                    let idx = if idx < 0 {
                        some!(s.chars().count().checked_sub(-idx as usize))
                    } else {
                        idx as usize
                    };
                    return s.chars().nth(idx).map(Value::from);
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        if let Some(idx) = key.as_i64() {
            let idx = some!(isize::try_from(idx).ok());
            let idx = if idx < 0 {
                some!(seq.item_count().checked_sub(-idx as usize))
            } else {
                idx as usize
            };
            seq.get_item(idx)
        } else {
            None
        }
    }

    /// Calls the value directly.
    ///
    /// If the value holds a function or macro, this invokes it.  Note that in
    /// MiniJinja there is a separate namespace for methods on objects and callable
    /// items.  To call methods (which should be a rather rare occurrence) you
    /// have to use [`call_method`](Self::call_method).
    ///
    /// The `args` slice is for the arguments of the function call.  To pass
    /// keyword arguments use the [`Kwargs`](crate::value::Kwargs) type.
    ///
    /// Usually the state is already available when it's useful to call this method,
    /// but when it's not available you can get a fresh template state straight
    /// from the [`Template`](crate::Template) via [`new_state`](crate::Template::new_state).
    ///
    /// ```
    /// # use minijinja::{Environment, value::{Value, Kwargs}};
    /// # let mut env = Environment::new();
    /// # env.add_template("foo", "").unwrap();
    /// # let tmpl = env.get_template("foo").unwrap();
    /// # let state = tmpl.new_state(); let state = &state;
    /// let func = Value::from_function(|v: i64, kwargs: Kwargs| {
    ///     v * kwargs.get::<i64>("mult").unwrap_or(1)
    /// });
    /// let rv = func.call(
    ///     state,
    ///     &[
    ///         Value::from(42),
    ///         Value::from(Kwargs::from_iter([("mult", Value::from(2))])),
    ///     ],
    /// ).unwrap();
    /// assert_eq!(rv, Value::from(84));
    /// ```
    ///
    /// With the [`args!`](crate::args) macro creating an argument slice is
    /// simplified:
    ///
    /// ```
    /// # use minijinja::{Environment, args, value::{Value, Kwargs}};
    /// # let mut env = Environment::new();
    /// # env.add_template("foo", "").unwrap();
    /// # let tmpl = env.get_template("foo").unwrap();
    /// # let state = tmpl.new_state(); let state = &state;
    /// let func = Value::from_function(|v: i64, kwargs: Kwargs| {
    ///     v * kwargs.get::<i64>("mult").unwrap_or(1)
    /// });
    /// let rv = func.call(state, args!(42, mult => 2)).unwrap();
    /// assert_eq!(rv, Value::from(84));
    /// ```
    pub fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        if let ValueBuf::Dynamic(ref dy) = self.0 {
            dy.call(state, args)
        } else {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("value of type {} is not callable", self.kind()),
            ))
        }
    }

    /// Calls a method on the value.
    ///
    /// The name of the method is `name`, the arguments passed are in the `args`
    /// slice.
    pub fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        match self.0 {
            ValueBuf::Dynamic(ref dy) => return dy.call_method(state, name, args),
            ValueBuf::Map(ref map, _) => {
                if let Some(value) = map.get_field(&name.into()) {
                    return value.call(state, args);
                }
            }
            _ => {}
        }
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("object has no method named {name}"),
        ))
    }

    /// Iterates over the value without holding a reference.
    pub(crate) fn try_iter_owned(&self) -> Result<OwnedValueIterator, Error> {
        let (iter_state, len) = match self.0 {
            ValueBuf::None | ValueBuf::Undefined => (ValueIteratorState::Empty, 0),
            ValueBuf::String(ref s, _) => (
                ValueIteratorState::Chars(0, Arc::clone(s)),
                s.chars().count(),
            ),
            ValueBuf::Seq(ref seq) => (
                ValueIteratorState::DynSeq(0, Arc::clone(seq)),
                seq.item_count(),
            ),
            ValueBuf::Map(ref s, _) => {
                // the assumption is that structs don't have excessive field counts
                // and that most iterations go over all fields, so creating a
                // temporary vector here is acceptable.
                if let Some(fields) = s.static_fields() {
                    (ValueIteratorState::StaticStr(0, fields), fields.len())
                } else {
                    let attrs = s.fields();
                    let attr_count = attrs.len();
                    (ValueIteratorState::Seq(0, Arc::from(attrs)), attr_count)
                }
            }
            ValueBuf::Dynamic(ref obj) => {
                match obj.kind() {
                    ObjectKind::Plain => (ValueIteratorState::Empty, 0),
                    ObjectKind::Seq(s) => todo!(),
                    //     // ValueIteratorState::DynSeq(0, Arc::clone(obj)),
                    //     s.item_count(),
                    // ),
                    ObjectKind::Struct(s) => {
                        // the assumption is that structs don't have excessive field counts
                        // and that most iterations go over all fields, so creating a
                        // temporary vector here is acceptable.
                        if let Some(fields) = s.static_fields() {
                            (ValueIteratorState::StaticStr(0, fields), fields.len())
                        } else {
                            let attrs = s.fields();
                            let attr_count = attrs.len();
                            (ValueIteratorState::Seq(0, Arc::from(attrs)), attr_count)
                        }
                    }
                }
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!("{} is not iterable", self.kind()),
                ))
            }
        };
        Ok(OwnedValueIterator { iter_state, len })
    }

    #[cfg(feature = "builtins")]
    pub(crate) fn get_path(&self, path: &str) -> Result<Value, Error> {
        let mut rv = self.clone();
        for part in path.split('.') {
            if let Ok(num) = part.parse::<usize>() {
                rv = ok!(rv.get_item_by_index(num));
            } else {
                rv = ok!(rv.get_attr(part));
            }
        }
        Ok(rv)
    }
}

impl Default for Value {
    fn default() -> Value {
        ValueBuf::Undefined.into()
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (ValueBuf::None, ValueBuf::None) => true,
            (ValueBuf::Undefined, ValueBuf::Undefined) => true,
            (ValueBuf::String(ref a, _), ValueBuf::String(ref b, _)) => a == b,
            (ValueBuf::Bytes(a), ValueBuf::Bytes(b)) => a == b,
            _ => match ops::coerce(self, other) {
                Some(ops::CoerceResult::F64(a, b)) => a == b,
                Some(ops::CoerceResult::I128(a, b)) => a == b,
                Some(ops::CoerceResult::Str(a, b)) => a == b,
                None => {
                    if let (Some(a), Some(b)) = (self.as_seq(), other.as_seq()) {
                        a.iter().eq(b.iter())
                    } else if self.kind() == ValueKind::Map && other.kind() == ValueKind::Map {
                        if self.len() != other.len() {
                            return false;
                        }
                        if let Ok(mut iter) = self.try_iter() {
                            iter.all(|x| self.get_item_opt(&x) == other.get_item_opt(&x))
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            },
        }
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        fn f64_total_cmp(left: f64, right: f64) -> Ordering {
            // this is taken from f64::total_cmp on newer rust versions
            let mut left = left.to_bits() as i64;
            let mut right = right.to_bits() as i64;
            left ^= (((left >> 63) as u64) >> 1) as i64;
            right ^= (((right >> 63) as u64) >> 1) as i64;
            left.cmp(&right)
        }

        let value_ordering = match (&self.0, &other.0) {
            (ValueBuf::None, ValueBuf::None) => Ordering::Equal,
            (ValueBuf::Undefined, ValueBuf::Undefined) => Ordering::Equal,
            (ValueBuf::String(ref a, _), ValueBuf::String(ref b, _)) => a.cmp(b),
            (ValueBuf::Bytes(a), ValueBuf::Bytes(b)) => a.cmp(b),
            _ => match ops::coerce(self, other) {
                Some(ops::CoerceResult::F64(a, b)) => f64_total_cmp(a, b),
                Some(ops::CoerceResult::I128(a, b)) => a.cmp(&b),
                Some(ops::CoerceResult::Str(a, b)) => a.cmp(b),
                None => {
                    if let (Some(a), Some(b)) = (self.as_seq(), other.as_seq()) {
                        a.iter().cmp(b.iter())
                    } else if self.kind() == ValueKind::Map && other.kind() == ValueKind::Map {
                        if let (Ok(a), Ok(b)) = (self.try_iter(), other.try_iter()) {
                            a.map(|k| (k.clone(), self.get_item_opt(&k)))
                                .cmp(b.map(|k| (k.clone(), other.get_item_opt(&k))))
                        } else {
                            Ordering::Equal
                        }
                    } else {
                        Ordering::Equal
                    }
                }
            },
        };
        value_ordering.then((self.kind() as usize).cmp(&(other.kind() as usize)))
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            ValueBuf::Undefined => Ok(()),
            ValueBuf::Bool(val) => val.fmt(f),
            ValueBuf::U64(val) => val.fmt(f),
            ValueBuf::I64(val) => val.fmt(f),
            ValueBuf::F64(val) => {
                if val.is_nan() {
                    f.write_str("NaN")
                } else if val.is_infinite() {
                    write!(f, "{}inf", if val.is_sign_negative() { "-" } else { "" })
                } else {
                    let mut num = val.to_string();
                    if !num.contains('.') {
                        num.push_str(".0");
                    }
                    write!(f, "{num}")
                }
            }
            ValueBuf::None => f.write_str("none"),
            ValueBuf::Invalid(ref val) => write!(f, "<invalid value: {}>", val),
            ValueBuf::I128(val) => write!(f, "{}", { val.0 }),
            ValueBuf::String(val, _) => write!(f, "{val}"),
            ValueBuf::Bytes(val) => write!(f, "{}", String::from_utf8_lossy(val)),
            ValueBuf::Seq(values) => {
                ok!(f.write_str("["));
                for (idx, val) in values.iter().enumerate() {
                    if idx > 0 {
                        ok!(f.write_str(", "));
                    }
                    ok!(write!(f, "{val:?}"));
                }
                f.write_str("]")
            }
            ValueBuf::Map(m, _) => {
                ok!(f.write_str("{"));
                for (idx, (key, val)) in m.iter().enumerate() {
                    if idx > 0 {
                        ok!(f.write_str(", "));
                    }
                    ok!(write!(f, "{key:?}: {val:?}"));
                }
                f.write_str("}")
            }
            ValueBuf::U128(val) => write!(f, "{}", { val.0 }),
            ValueBuf::Dynamic(x) => write!(f, "{x}"),
        }
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.0 {
            ValueBuf::None | ValueBuf::Undefined => 0u8.hash(state),
            ValueBuf::String(ref s, _) => s.hash(state),
            ValueBuf::Bool(b) => b.hash(state),
            ValueBuf::Invalid(s) => s.hash(state),
            ValueBuf::Bytes(b) => b.hash(state),
            ValueBuf::Seq(b) => b.hash(state),
            ValueBuf::Map(m, _) => m.iter().for_each(|(k, v)| {
                k.hash(state);
                v.hash(state);
            }),
            ValueBuf::Dynamic(d) => match d.kind() {
                ObjectKind::Plain => 0u8.hash(state),
                ObjectKind::Seq(s) => s.iter().for_each(|x| x.hash(state)),
                ObjectKind::Struct(s) => {
                    if let Some(fields) = s.static_fields() {
                        fields.iter().for_each(|k| {
                            k.hash(state);
                            s.get_field(&Value::from(*k)).hash(state);
                        });
                    } else {
                        s.fields().iter().for_each(|k| {
                            k.hash(state);
                            s.get_field(k).hash(state);
                        });
                    }
                }
            },
            ValueBuf::U64(_)
            | ValueBuf::I64(_)
            | ValueBuf::F64(_)
            | ValueBuf::U128(_)
            | ValueBuf::I128(_) => {
                if let Ok(val) = i64::try_from(self.clone()) {
                    val.hash(state)
                } else {
                    ops::as_f64(self).map(|x| x.to_bits()).hash(state)
                }
            }
        }
    }
}

/// Iterates over a value.
pub struct ValueIter<'a> {
    _marker: PhantomData<&'a Value>,
    inner: OwnedValueIterator,
}

impl<'a> Iterator for ValueIter<'a> {
    type Item = Value;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub(crate) struct OwnedValueIterator {
    iter_state: ValueIteratorState,
    len: usize,
}

impl Iterator for OwnedValueIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter_state.advance_state().map(|x| {
            self.len -= 1;
            x
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl ExactSizeIterator for OwnedValueIterator {}

impl fmt::Debug for OwnedValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueIterator").finish()
    }
}

enum ValueIteratorState {
    Empty,
    Chars(usize, Arc<str>),
    Seq(usize, Arc<[Value]>),
    StaticStr(usize, &'static [&'static str]),
    ArcStr(usize, Vec<Arc<str>>),
    DynSeq(usize, Arc<dyn SeqObject>),
    #[cfg(not(feature = "preserve_order"))]
    Map(Option<Value>, Arc<OwnedValueMap>),
    #[cfg(feature = "preserve_order")]
    Map(usize, Arc<OwnedValueMap>),
}

impl ValueIteratorState {
    fn advance_state(&mut self) -> Option<Value> {
        match self {
            ValueIteratorState::Empty => None,
            ValueIteratorState::Chars(offset, ref s) => {
                (s as &str)[*offset..].chars().next().map(|c| {
                    *offset += c.len_utf8();
                    Value::from(c)
                })
            }
            ValueIteratorState::Seq(idx, items) => items
                .get(*idx)
                .map(|x| {
                    *idx += 1;
                    x
                })
                .cloned(),
            ValueIteratorState::StaticStr(idx, items) => items.get(*idx).map(|x| {
                *idx += 1;
                Value::from(intern(x))
            }),
            ValueIteratorState::ArcStr(idx, items) => items.get(*idx).map(|x| {
                *idx += 1;
                Value::from(x.clone())
            }),
            ValueIteratorState::DynSeq(idx, seq) => {
                seq.get_item(*idx).map(|x| {
                    *idx += 1;
                    x
                })
            }
            #[cfg(feature = "preserve_order")]
            ValueIteratorState::Map(idx, map) => map.get_index(*idx).map(|x| {
                *idx += 1;
                x.0.clone()
            }),
            #[cfg(not(feature = "preserve_order"))]
            ValueIteratorState::Map(ptr, map) => {
                if let Some(current) = ptr.take() {
                    let next = map.get_key_value(&current).map(|x| x.0.clone());
                    let rv = current;
                    *ptr = next;
                    Some(rv)
                } else {
                    None
                }
            }
        }
    }
}

impl fmt::Debug for ValueBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueBuf::Undefined => f.write_str("Undefined"),
            ValueBuf::Bool(val) => fmt::Debug::fmt(val, f),
            ValueBuf::U64(val) => fmt::Debug::fmt(val, f),
            ValueBuf::I64(val) => fmt::Debug::fmt(val, f),
            ValueBuf::F64(val) => fmt::Debug::fmt(val, f),
            ValueBuf::None => f.write_str("None"),
            ValueBuf::Invalid(ref val) => write!(f, "<invalid value: {}>", val),
            ValueBuf::U128(val) => fmt::Debug::fmt(&{ val.0 }, f),
            ValueBuf::I128(val) => fmt::Debug::fmt(&{ val.0 }, f),
            ValueBuf::String(val, _) => fmt::Debug::fmt(val, f),
            ValueBuf::Bytes(val) => fmt::Debug::fmt(val, f),
            ValueBuf::Seq(val) => fmt::Debug::fmt(val, f),
            ValueBuf::Map(val, _) => fmt::Debug::fmt(val, f),
            ValueBuf::Dynamic(val) => fmt::Debug::fmt(val, f),
        }
    }
}

impl<T: Copy> Clone for Packed<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            ValueKind::Undefined => "undefined",
            ValueKind::None => "none",
            ValueKind::Bool => "bool",
            ValueKind::Number => "number",
            ValueKind::String => "string",
            ValueKind::Bytes => "bytes",
            ValueKind::Seq => "sequence",
            ValueKind::Map => "map",
        })
    }
}
