use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use crate::functions;
use crate::error::{Error, ErrorKind};
use crate::vm::State;

use crate::value::ops;
use crate::value::intern;

use crate::value::ValueBox;
use crate::value::argtypes::{FunctionArgs, FunctionResult};
use crate::value::object::{Object, SeqObject, MapObject};

use super::Value;

#[derive(Debug, Clone)]
pub enum ValueRepr<'a> {
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
    Seq(ArcCow<'a, dyn SeqObject + 'static>),
    Map(ArcCow<'a, dyn MapObject + 'static>, MapType),
    Dynamic(ArcCow<'a, dyn Object + 'static>),
}

impl Value<'_> {
    pub fn into_owned(self) -> ValueBox {
        let repr = match self.0 {
            ValueRepr::None => ValueRepr::None,
            ValueRepr::Undefined => ValueRepr::Undefined,
            ValueRepr::Bool(v) => ValueRepr::Bool(v),
            ValueRepr::U64(v) => ValueRepr::U64(v),
            ValueRepr::I64(v) => ValueRepr::I64(v),
            ValueRepr::F64(v) => ValueRepr::F64(v),
            ValueRepr::U128(v) => ValueRepr::U128(v),
            ValueRepr::I128(v) => ValueRepr::I128(v),
            ValueRepr::Invalid(v) => ValueRepr::Invalid(v.to_owned()),
            ValueRepr::String(v, k) => ValueRepr::String(v.to_owned(), k) ,
            ValueRepr::Bytes(v) => ValueRepr::Bytes(v.to_owned()),
            ValueRepr::Seq(v) => ValueRepr::Seq(match v {
                ArcCow::Borrowed(v) => ArcCow::Owned(v.cloned().into()),
                ArcCow::Owned(v) => ArcCow::Owned(v),
            }),
            ValueRepr::Map(v, k) => ValueRepr::Map(match v {
                ArcCow::Borrowed(v) => ArcCow::Owned(v.cloned().into()),
                ArcCow::Owned(v) => ArcCow::Owned(v),
            }, k),
            ValueRepr::Dynamic(v) => ValueRepr::Dynamic(match v {
                ArcCow::Borrowed(v) => ArcCow::Owned(v.cloned().into()),
                ArcCow::Owned(v) => ArcCow::Owned(v),
            }),
        };

        Value(repr)
    }
}

impl<T: ?Sized> std::ops::Deref for ArcCow<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            ArcCow::Borrowed(v) => v,
            ArcCow::Owned(v) => &*v,
        }
    }
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

impl<T: ?Sized> ArcCow<'_, T> where for<'a> Arc<T>: From<&'a T> {
    pub fn to_owned(&self) -> ArcCow<'static, T> {
        match self {
            ArcCow::Borrowed(v) => ArcCow::Owned(Arc::from(v)),
            ArcCow::Owned(v) => ArcCow::Owned(v.clone()),
        }
    }
}

impl<'a, 'b, B: ?Sized, A: PartialEq<B> + ?Sized> PartialEq<ArcCow<'b, B>> for ArcCow<'a, A> {
    fn eq(&self, other: &ArcCow<'b, B>) -> bool {
        self.deref() == other.deref()
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for ArcCow<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
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
    pub fn borrowed(value: &'a T) -> ArcCow<'a, T> {
        ArcCow::Borrowed(value)
    }
}

impl<'a, T: 'a> ArcCow<'a, T> {
    pub fn owned(value: T) -> ArcCow<'a, T> {
        ArcCow::Owned(Arc::new(value))
    }
}

impl<'a, V: ?Sized, T: Into<Arc<V>> + ?Sized> From<T> for ArcCow<'a, V> {
    fn from(value: T) -> Self {
        ArcCow::Owned(value.into())
    }
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
impl Value<'_> {
    /// The undefined value.
    ///
    /// This constant exists because the undefined type does not exist in Rust
    /// and this is the only way to construct it.
    pub const UNDEFINED: ValueBox = Value(ValueRepr::Undefined);

    pub const NONE: ValueBox = Value(ValueRepr::None);

    /// Creates a value from a safe string.
    ///
    /// A safe string is one that will bypass auto escaping.  For instance if you
    /// want to have the template engine render some HTML without the user having to
    /// supply the `|safe` filter, you can use a value of this type instead.
    ///
    /// ```
    /// # use minijinja::value::ValueBox;
    /// let val = ValueBox::from_safe_string("<em>note</em>".into());
    /// ```
    pub fn from_safe_string(value: String) -> ValueBox {
        ValueRepr::String(ArcCow::from(value), StringType::Safe).into()
    }

    /// Creates a value from a dynamic object.
    ///
    /// For more information see [`Object`].
    ///
    /// ```rust
    /// # use minijinja::value::{ValueBox, Object};
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
    /// let val = ValueBox::from_object(Thing { id: 42 });
    /// ```
    ///
    /// Objects are internally reference counted.  If you want to hold on to the
    /// `Arc` you can directly create the value from an arc'ed object:
    ///
    /// ```rust
    /// # use minijinja::value::{ValueBox, Object};
    /// # #[derive(Debug)]
    /// # struct Thing { id: usize };
    /// # impl std::fmt::Display for Thing {
    /// #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    /// #         todo!();
    /// #     }
    /// # }
    /// # impl Object for Thing {}
    /// use std::sync::Arc;
    /// let val = ValueBox::from(Arc::new(Thing { id: 42 }));
    /// ```
    pub fn from_object<'a, T: Object + ?Sized + 'a>(value: T) -> Value<'a>
        where T: Into<Arc<T>>
    {
        let arc = value.into();
        Value(ValueRepr::Dynamic(ArcCow::Owned(arc)))
    }

    /// Creates a value from an owned [`SeqObject`].
    ///
    /// This is a simplified API for creating dynamic sequences
    /// without having to implement the entire [`Object`] protocol.
    ///
    /// **Note:** objects created this way cannot be downcasted via
    /// [`downcast_object_ref`](Self::downcast_object_ref).
    pub fn from_seq_object<T: SeqObject + ?Sized + 'static>(value: T) -> ValueBox
        where T: Into<Arc<T>>
    {
        let arc = value.into() as Arc<dyn SeqObject>;
        Value(ValueRepr::Seq(ArcCow::Owned(arc)))
    }

    pub fn from_seq_ref<'a, T: SeqObject + ?Sized + 'static>(value: &'a T) -> Value<'a>
        where T: Into<Arc<T>>
    {
        let arc = ArcCow::Borrowed(value as &dyn SeqObject);
        Value(ValueRepr::Seq(arc))
    }

    /// Creates a value from an owned [`MapObject`].
    ///
    /// This is a simplified API for creating dynamic structs
    /// without having to implement the entire [`Object`] protocol.
    ///
    /// **Note:** objects created this way cannot be downcasted via
    /// [`downcast_object_ref`](Self::downcast_object_ref).
    pub fn from_map_object<T: MapObject + ?Sized + 'static>(value: T) -> ValueBox
        where T: Into<Arc<T>>
    {
        ValueRepr::Map(ArcCow::Owned(value.into()), MapType::Normal).into()
    }

    pub fn from_map_ref<'a, T: MapObject + ?Sized + 'static>(value: &'a T) -> Value<'a>
        where T: Into<Arc<T>>
    {
        let arc = ArcCow::Borrowed(value as &dyn MapObject);
        Value(ValueRepr::Map(arc, MapType::Normal))
    }

    pub(crate) fn from_kwargs<T: MapObject + ?Sized + 'static>(value: T) -> ValueBox
        where T: Into<Arc<T>>
    {
        Value(ValueRepr::Map(ArcCow::Owned(value.into()), MapType::Kwargs))
    }

    /// Creates a callable value from a function.
    ///
    /// ```
    /// # use minijinja::value::ValueBox;
    /// let pow = ValueBox::from_function(|a: u32| a * a);
    /// ```
    pub fn from_function<F, Rv, Args>(f: F) -> ValueBox
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
            ValueRepr::Undefined => ValueKind::Undefined,
            ValueRepr::Bool(_) => ValueKind::Bool,
            ValueRepr::U64(_) | ValueRepr::I64(_) | ValueRepr::F64(_) => ValueKind::Number,
            ValueRepr::None => ValueKind::None,
            ValueRepr::I128(_) => ValueKind::Number,
            ValueRepr::String(..) => ValueKind::String,
            ValueRepr::Bytes(_) => ValueKind::Bytes,
            ValueRepr::U128(_) => ValueKind::Number,
            ValueRepr::Seq(_) => ValueKind::Seq,
            ValueRepr::Map(..) => ValueKind::Map,
            // XXX: invalid values report themselves as maps which is a lie
            ValueRepr::Invalid(_) => ValueKind::Map,
            ValueRepr::Dynamic(ref dy) => dy.value().kind(),
        }
    }

    /// Returns `true` if the value is a number.
    ///
    /// To convert a value into a primitive number, use [`TryFrom`] or [`TryInto`].
    pub fn is_number(&self) -> bool {
        matches!(
            self.0,
            ValueRepr::U64(_)
                | ValueRepr::I64(_)
                | ValueRepr::F64(_)
                | ValueRepr::I128(_)
                | ValueRepr::U128(_)
        )
    }

    /// Returns `true` if the map represents keyword arguments.
    pub fn is_kwargs(&self) -> bool {
        matches!(self.0, ValueRepr::Map(_, MapType::Kwargs))
    }

    /// Is this value true?
    pub fn is_true(&self) -> bool {
        match self.0 {
            ValueRepr::Bool(val) => val,
            ValueRepr::U64(x) => x != 0,
            ValueRepr::U128(x) => x.0 != 0,
            ValueRepr::I64(x) => x != 0,
            ValueRepr::I128(x) => x.0 != 0,
            ValueRepr::F64(x) => x != 0.0,
            ValueRepr::String(ref x, _) => !x.is_empty(),
            ValueRepr::Bytes(ref x) => !x.is_empty(),
            ValueRepr::None | ValueRepr::Undefined | ValueRepr::Invalid(_) => false,
            ValueRepr::Seq(ref x) => x.item_count() != 0,
            ValueRepr::Map(ref x, _) => x.field_count() != 0,
            ValueRepr::Dynamic(ref x) => x.value().is_true(),
        }
    }

    /// Returns `true` if this value is safe.
    pub fn is_safe(&self) -> bool {
        matches!(&self.0, ValueRepr::String(_, StringType::Safe))
    }

    /// Returns `true` if this value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(&self.0, ValueRepr::Undefined)
    }

    /// Returns `true` if this value is none.
    pub fn is_none(&self) -> bool {
        matches!(&self.0, ValueRepr::None)
    }

    /// If the value is a string, return it.
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            ValueRepr::String(ref s, _) => Some(s as &str),
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
            ValueRepr::String(ref s, _) => Some(s.as_bytes()),
            ValueRepr::Bytes(ref b) => Some(&b[..]),
            _ => None,
        }
    }

    /// If the value is an object, it's returned as [`Object`].
    pub fn as_object(&self) -> Option<&dyn Object> {
        match self.0 {
            ValueRepr::Dynamic(ref dy) => Some(&**dy as &dyn Object),
            _ => None,
        }
    }

    /// If the value is a sequence it's returned as [`SeqObject`].
    pub fn as_seq(&self) -> Option<&dyn SeqObject> {
        match self.0 {
            ValueRepr::Seq(ref v) => Some(&**v),
            ValueRepr::Dynamic(ref dy) => match dy.value() {
                Value(ValueRepr::Seq(ArcCow::Borrowed(v))) => Some(v),
                _ => None
            }
            _ => None
        }
    }

    /// If the value is a struct, return it as [`MapObject`].
    pub fn as_struct(&self) -> Option<&dyn MapObject> {
        match self.0 {
            ValueRepr::Map(ref v, _) => Some(&**v),
            ValueRepr::Dynamic(ref dy) => match dy.value() {
                Value(ValueRepr::Map(ArcCow::Borrowed(v), _)) => Some(v),
                _ => None
            }
            _ => None
        }
    }

    /// Returns the length of the contained value.
    ///
    /// ValueBoxs without a length will return `None`.
    ///
    /// ```
    /// # use minijinja::value::ValueBox;
    /// let seq = ValueBox::from(vec![1, 2, 3, 4]);
    /// assert_eq!(seq.len(), Some(4));
    /// ```
    pub fn len(&self) -> Option<usize> {
        match self.0 {
            ValueRepr::String(ref s, _) => Some(s.chars().count()),
            ValueRepr::Map(ref items, _) => Some(items.field_count()),
            ValueRepr::Seq(ref items) => Some(items.item_count()),
            ValueRepr::Dynamic(ref dy) => dy.value().len(),
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
    /// # use minijinja::value::ValueBox;
    /// # fn test() -> Result<(), minijinja::Error> {
    /// let ctx = minijinja::context! {
    ///     foo => "Foo"
    /// };
    /// let value = ctx.get_attr("foo")?;
    /// assert_eq!(value.to_string(), "Foo");
    /// # Ok(()) }
    /// ```
    pub fn get_attr(&self, key: &str) -> Result<ValueBox, Error> {
        Ok(match self.0 {
            ValueRepr::Undefined => return Err(Error::from(ErrorKind::UndefinedError)),
            ValueRepr::Map(ref items, _) => items.get_field(&ValueBox::from(key)),
            ValueRepr::Dynamic(ref dy) => return dy.value().get_attr(key),
            _ => None,
        }
        .unwrap_or(ValueBox::UNDEFINED))
    }

    /// Alternative lookup strategy without error handling exclusively for context
    /// resolution.
    ///
    /// The main difference is that the return value will be `None` if the value is
    /// unable to look up the key rather than returning `Undefined` and errors will
    /// also not be created.
    pub(crate) fn get_attr_fast(&self, key: &str) -> Option<ValueBox> {
        match self.0 {
            ValueRepr::Map(ref items, _) => items.get_field(&key.into()),
            ValueRepr::Dynamic(ref dy) => dy.value().get_attr_fast(key),
            _ => None,
        }
    }

    /// Looks up an index of the value.
    ///
    /// This is a shortcut for [`get_item`](Self::get_item).
    ///
    /// ```
    /// # use minijinja::value::ValueBox;
    /// let seq = ValueBox::from(vec![0u32, 1, 2]);
    /// let value = seq.get_item_by_index(1).unwrap();
    /// assert_eq!(value.try_into().ok(), Some(1));
    /// ```
    pub fn get_item_by_index(&self, idx: usize) -> Result<ValueBox, Error> {
        self.get_item(&Value(ValueRepr::U64(idx as _)))
    }

    /// Looks up an item (or attribute) by key.
    ///
    /// This is similar to [`get_attr`](Self::get_attr) but instead of using
    /// a string key this can be any key.  For instance this can be used to
    /// index into sequences.  Like [`get_attr`](Self::get_attr) this returns
    /// [`UNDEFINED`](Self::UNDEFINED) when an invalid key is looked up.
    ///
    /// ```
    /// # use minijinja::value::ValueBox;
    /// let ctx = minijinja::context! {
    ///     foo => "Foo",
    /// };
    /// let value = ctx.get_item(&ValueBox::from("foo")).unwrap();
    /// assert_eq!(value.to_string(), "Foo");
    /// ```
    pub fn get_item(&self, key: &ValueBox) -> Result<ValueBox, Error> {
        if let ValueRepr::Undefined = self.0 {
            Err(Error::from(ErrorKind::UndefinedError))
        } else {
            Ok(self.get_item_opt(key).unwrap_or(ValueBox::UNDEFINED))
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
    /// # use minijinja::value::ValueBox;
    /// # fn test() -> Result<(), minijinja::Error> {
    /// let value = ValueBox::from({
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
    /// # use minijinja::value::{ValueBox, Object};
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
    /// let x_value = ValueBox::from_object(Thing { id: 42 });
    /// let thing = x_value.downcast_object_ref::<Thing>().unwrap();
    /// assert_eq!(thing.id, 42);
    /// ```
    pub fn downcast_object_ref<T: Object>(&self) -> Option<&T> {
        self.as_object().and_then(|x| x.downcast_ref())
    }

    pub(crate) fn get_item_opt(&self, key: &ValueBox) -> Option<ValueBox> {
        let seq = match self.0 {
            ValueRepr::Map(ref items, _) => return items.get_field(key),
            ValueRepr::Seq(ref items) => &**items as &dyn SeqObject,
            ValueRepr::Dynamic(ref dy) => return dy.value().get_item_opt(key),
            ValueRepr::String(ref s, _) => {
                if let Some(idx) = key.as_i64() {
                    let idx = some!(isize::try_from(idx).ok());
                    let idx = if idx < 0 {
                        some!(s.chars().count().checked_sub(-idx as usize))
                    } else {
                        idx as usize
                    };
                    return s.chars().nth(idx).map(ValueBox::from);
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
    /// # use minijinja::{Environment, value::{ValueBox, Kwargs}};
    /// # let mut env = Environment::new();
    /// # env.add_template("foo", "").unwrap();
    /// # let tmpl = env.get_template("foo").unwrap();
    /// # let state = tmpl.new_state(); let state = &state;
    /// let func = ValueBox::from_function(|v: i64, kwargs: Kwargs| {
    ///     v * kwargs.get::<i64>("mult").unwrap_or(1)
    /// });
    /// let rv = func.call(
    ///     state,
    ///     &[
    ///         ValueBox::from(42),
    ///         ValueBox::from(Kwargs::from_iter([("mult", ValueBox::from(2))])),
    ///     ],
    /// ).unwrap();
    /// assert_eq!(rv, ValueBox::from(84));
    /// ```
    ///
    /// With the [`args!`](crate::args) macro creating an argument slice is
    /// simplified:
    ///
    /// ```
    /// # use minijinja::{Environment, args, value::{ValueBox, Kwargs}};
    /// # let mut env = Environment::new();
    /// # env.add_template("foo", "").unwrap();
    /// # let tmpl = env.get_template("foo").unwrap();
    /// # let state = tmpl.new_state(); let state = &state;
    /// let func = ValueBox::from_function(|v: i64, kwargs: Kwargs| {
    ///     v * kwargs.get::<i64>("mult").unwrap_or(1)
    /// });
    /// let rv = func.call(state, args!(42, mult => 2)).unwrap();
    /// assert_eq!(rv, ValueBox::from(84));
    /// ```
    pub fn call(&self, state: &State, args: &[ValueBox]) -> Result<ValueBox, Error> {
        if let ValueRepr::Dynamic(ref dy) = self.0 {
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
    pub fn call_method(&self, state: &State, name: &str, args: &[ValueBox]) -> Result<ValueBox, Error> {
        match self.0 {
            ValueRepr::Dynamic(ref dy) => return dy.call_method(state, name, args),
            ValueRepr::Map(ref map, _) => {
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
        fn make_it_owned(
            value: &ArcCow<'_, dyn SeqObject + 'static>
        ) -> ArcCow<'static, dyn SeqObject + 'static> {
            match value {
                ArcCow::Borrowed(v) => ArcCow::Owned(v.cloned()),
                ArcCow::Owned(v) => ArcCow::Owned(v.clone()),
            }
        }

        let (iter_state, len) = match self.0 {
            ValueRepr::None | ValueRepr::Undefined => (ValueIteratorState::Empty, 0),
            ValueRepr::String(ref s, _) =>  {
                let v: ArcCow<'static, str> = s.to_owned();
                (ValueIteratorState::Chars(0, v), s.chars().count())
            },
            ValueRepr::Seq(ref seq) => {
                let v: ArcCow<'static, dyn SeqObject + 'static> = make_it_owned(seq);
                (ValueIteratorState::DynSeq(0, v), seq.item_count())
            },
            ValueRepr::Map(ref s, _) => {
                // the assumption is that structs don't have excessive field counts
                // and that most iterations go over all fields, so creating a
                // temporary vector here is acceptable.
                if let Some(fields) = s.static_fields() {
                    (ValueIteratorState::StaticStr(0, fields), fields.len())
                } else {
                    let attrs = s.fields();
                    let attr_count = attrs.len();
                    (ValueIteratorState::Seq(0, ArcCow::from(attrs)), attr_count)
                }
            }
            ValueRepr::Dynamic(ref obj) => return obj.value().try_iter_owned(),
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
    pub(crate) fn get_path(&self, path: &str) -> Result<ValueBox, Error> {
        let mut rv = self.clone();
        for part in path.split('.') {
            if let Ok(num) = part.parse::<usize>() {
                rv = ok!(rv.get_item_by_index(num));
            } else {
                rv = ok!(rv.get_attr(part));
            }
        }
        Ok(rv.into_owned())
    }
}

impl Default for Value<'_> {
    fn default() -> ValueBox {
        Value::UNDEFINED
    }
}

impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (ValueRepr::None, ValueRepr::None) => true,
            (ValueRepr::Undefined, ValueRepr::Undefined) => true,
            (ValueRepr::String(ref a, _), ValueRepr::String(ref b, _)) => a == b,
            (ValueRepr::Bytes(a), ValueRepr::Bytes(b)) => a == b,
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

impl Eq for Value<'_> {}

impl PartialOrd for Value<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value<'_> {
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
            (ValueRepr::None, ValueRepr::None) => Ordering::Equal,
            (ValueRepr::Undefined, ValueRepr::Undefined) => Ordering::Equal,
            (ValueRepr::String(ref a, _), ValueRepr::String(ref b, _)) => a.cmp(b),
            (ValueRepr::Bytes(a), ValueRepr::Bytes(b)) => a.cmp(b),
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

impl fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            ValueRepr::Undefined => Ok(()),
            ValueRepr::Bool(val) => val.fmt(f),
            ValueRepr::U64(val) => val.fmt(f),
            ValueRepr::I64(val) => val.fmt(f),
            ValueRepr::F64(val) => {
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
            ValueRepr::None => f.write_str("none"),
            ValueRepr::Invalid(ref val) => write!(f, "<invalid value: {}>", val),
            ValueRepr::I128(val) => write!(f, "{}", { val.0 }),
            ValueRepr::String(val, _) => write!(f, "{val}"),
            ValueRepr::Bytes(val) => write!(f, "{}", String::from_utf8_lossy(val)),
            ValueRepr::Seq(values) => {
                ok!(f.write_str("["));
                for (idx, val) in values.iter().enumerate() {
                    if idx > 0 {
                        ok!(f.write_str(", "));
                    }
                    ok!(write!(f, "{val:?}"));
                }
                f.write_str("]")
            }
            ValueRepr::Map(m, _) => {
                ok!(f.write_str("{"));
                for (idx, (key, val)) in m.iter().enumerate() {
                    if idx > 0 {
                        ok!(f.write_str(", "));
                    }
                    ok!(write!(f, "{key:?}: {val:?}"));
                }
                f.write_str("}")
            }
            ValueRepr::U128(val) => write!(f, "{}", { val.0 }),
            ValueRepr::Dynamic(x) => write!(f, "{x}"),
        }
    }
}

impl Hash for Value<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.0 {
            ValueRepr::None | ValueRepr::Undefined => 0u8.hash(state),
            ValueRepr::String(ref s, _) => s.hash(state),
            ValueRepr::Bool(b) => b.hash(state),
            ValueRepr::Invalid(s) => s.hash(state),
            ValueRepr::Bytes(b) => b.hash(state),
            ValueRepr::Seq(b) => b.hash(state),
            ValueRepr::Map(m, _) => m.iter().for_each(|(k, v)| {
                k.hash(state);
                v.hash(state);
            }),
            ValueRepr::Dynamic(d) => d.value().hash(state),
            ValueRepr::U64(_)
            | ValueRepr::I64(_)
            | ValueRepr::F64(_)
            | ValueRepr::U128(_)
            | ValueRepr::I128(_) => {
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
    _marker: PhantomData<&'a ValueBox>,
    inner: OwnedValueIterator,
}

impl<'a> Iterator for ValueIter<'a> {
    type Item = ValueBox;

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
    type Item = ValueBox;

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
    Chars(usize, ArcCow<'static, str>),
    Seq(usize, ArcCow<'static, [ValueBox]>),
    StaticStr(usize, &'static [&'static str]),
    DynSeq(usize, ArcCow<'static, dyn SeqObject>),
}

impl ValueIteratorState {
    fn advance_state(&mut self) -> Option<ValueBox> {
        match self {
            ValueIteratorState::Empty => None,
            ValueIteratorState::Chars(offset, ref s) => {
                (s as &str)[*offset..].chars().next().map(|c| {
                    *offset += c.len_utf8();
                    ValueBox::from(c)
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
                ValueBox::from(intern(x))
            }),
            ValueIteratorState::DynSeq(idx, seq) => {
                seq.get_item(*idx).map(|x| {
                    *idx += 1;
                    x
                })
            }
        }
    }
}

// impl fmt::Debug for ValueRepr {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             ValueRepr::Undefined => f.write_str("Undefined"),
//             ValueRepr::Bool(val) => fmt::Debug::fmt(val, f),
//             ValueRepr::U64(val) => fmt::Debug::fmt(val, f),
//             ValueRepr::I64(val) => fmt::Debug::fmt(val, f),
//             ValueRepr::F64(val) => fmt::Debug::fmt(val, f),
//             ValueRepr::None => f.write_str("None"),
//             ValueRepr::Invalid(ref val) => write!(f, "<invalid value: {}>", val),
//             ValueRepr::U128(val) => fmt::Debug::fmt(&{ val.0 }, f),
//             ValueRepr::I128(val) => fmt::Debug::fmt(&{ val.0 }, f),
//             ValueRepr::String(val, _) => fmt::Debug::fmt(val, f),
//             ValueRepr::Bytes(val) => fmt::Debug::fmt(val, f),
//             ValueRepr::Seq(val) => fmt::Debug::fmt(val, f),
//             ValueRepr::Map(val, _) => fmt::Debug::fmt(val, f),
//             ValueRepr::Dynamic(val) => fmt::Debug::fmt(val, f),
//         }
//     }
// }

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
