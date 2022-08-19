//! Filter functions and abstractions.
//!
//! MiniJinja inherits from Jinja2 the concept of filter functions.  These are functions
//! which are applied to values to modify them.  For example the expression `{{ 42|filter(23) }}`
//! invokes the filter `filter` with the arguments `42` and `23`.
//!
//! MiniJinja comes with some built-in filters that are listed below. To create a
//! custom filter write a function that takes at least a
//! [`&State`](crate::State) and value argument, then register it
//! with [`add_filter`](crate::Environment::add_filter).
//!
//! # Using Filters
//!
//! Using filters in templates is possible in all places an expression is permitted.
//! This means they are not just used for printing but also are useful for iteration
//! or similar situations.
//!
//! Motivating example:
//!
//! ```jinja
//! <dl>
//! {% for key, value in config|items %}
//!   <dt>{{ key }}
//!   <dd><pre>{{ value|tojson }}</pre>
//! {% endfor %}
//! </dl>
//! ```
//!
//! # Custom Filters
//!
//! A custom filter is just a simple function which accepts inputs as parameters and then
//! returns a new value.  For instance the following shows a filter which takes an input
//! value and replaces whitespace with dashes and converts it to lowercase:
//!
//! ```
//! # use minijinja::{Environment, State, Error};
//! # let mut env = Environment::new();
//! fn slugify(_state: &State, value: String) -> Result<String, Error> {
//!     Ok(value.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-"))
//! }
//!
//! env.add_filter("slugify", slugify);
//! ```
//!
//! MiniJinja will perform the necessary conversions automatically via the
//! [`FunctionArgs`](crate::value::FunctionArgs) and [`Into`] traits.
use std::collections::BTreeMap;

use crate::error::Error;
use crate::utils::HtmlEscape;
use crate::value::{ArgType, FunctionArgs, RcType, Value};
use crate::vm::State;

type FilterFunc = dyn Fn(&State, Value, Vec<Value>) -> Result<Value, Error> + Sync + Send + 'static;

#[derive(Clone)]
pub(crate) struct BoxedFilter(RcType<FilterFunc>);

/// A utility trait that represents filters.
pub trait Filter<V = Value, Rv = Value, Args = Vec<Value>>: Send + Sync + 'static {
    /// Applies a filter to value with the given arguments.
    fn apply_to(&self, state: &State, value: V, args: Args) -> Result<Rv, Error>;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, V, Rv, $($name),*> Filter<V, Rv, ($($name,)*)> for Func
        where
            Func: Fn(&State, V, $($name),*) -> Result<Rv, Error> + Send + Sync + 'static
        {
            fn apply_to(&self, state: &State, value: V, args: ($($name,)*)) -> Result<Rv, Error> {
                #[allow(non_snake_case)]
                let ($($name,)*) = args;
                (self)(state, value, $($name,)*)
            }
        }
    };
}

tuple_impls! {}
tuple_impls! { A }
tuple_impls! { A B }
tuple_impls! { A B C }
tuple_impls! { A B C D }

impl BoxedFilter {
    /// Creates a new boxed filter.
    pub fn new<F, V, Rv, Args>(f: F) -> BoxedFilter
    where
        F: Filter<V, Rv, Args>,
        V: ArgType,
        Rv: Into<Value>,
        Args: FunctionArgs,
    {
        BoxedFilter(RcType::new(
            move |state, value, args| -> Result<Value, Error> {
                f.apply_to(
                    state,
                    ArgType::from_value(Some(value))?,
                    FunctionArgs::from_values(args)?,
                )
                .map(Into::into)
            },
        ))
    }

    /// Applies the filter to a value and argument.
    pub fn apply_to(&self, state: &State, value: Value, args: Vec<Value>) -> Result<Value, Error> {
        (self.0)(state, value, args)
    }
}

pub(crate) fn get_builtin_filters() -> BTreeMap<&'static str, BoxedFilter> {
    let mut rv = BTreeMap::new();
    rv.insert("safe", BoxedFilter::new(safe));
    rv.insert("escape", BoxedFilter::new(escape));
    rv.insert("e", BoxedFilter::new(escape));
    #[cfg(feature = "builtins")]
    {
        rv.insert("lower", BoxedFilter::new(lower));
        rv.insert("upper", BoxedFilter::new(upper));
        rv.insert("title", BoxedFilter::new(title));
        rv.insert("replace", BoxedFilter::new(replace));
        rv.insert("length", BoxedFilter::new(length));
        rv.insert("count", BoxedFilter::new(length));
        rv.insert("dictsort", BoxedFilter::new(dictsort));
        rv.insert("items", BoxedFilter::new(items));
        rv.insert("reverse", BoxedFilter::new(reverse));
        rv.insert("trim", BoxedFilter::new(trim));
        rv.insert("join", BoxedFilter::new(join));
        rv.insert("default", BoxedFilter::new(default));
        rv.insert("round", BoxedFilter::new(round));
        rv.insert("abs", BoxedFilter::new(abs));
        rv.insert("first", BoxedFilter::new(first));
        rv.insert("last", BoxedFilter::new(last));
        rv.insert("d", BoxedFilter::new(default));
        rv.insert("list", BoxedFilter::new(list));
        rv.insert("bool", BoxedFilter::new(bool));
        rv.insert("batch", BoxedFilter::new(batch));
        rv.insert("slice", BoxedFilter::new(slice));
        #[cfg(feature = "json")]
        {
            rv.insert("tojson", BoxedFilter::new(tojson));
        }
        #[cfg(feature = "urlencode")]
        {
            rv.insert("urlencode", BoxedFilter::new(urlencode));
        }
    }
    rv
}

/// Marks a value as safe.  This converts it into a string.
pub fn safe(_state: &State, v: String) -> Result<Value, Error> {
    // TODO: this ideally understands which type of escaping is in use
    Ok(Value::from_safe_string(v))
}

/// HTML escapes a string.
///
/// By default this filter is also registered under the alias `e`.
pub fn escape(_state: &State, v: Value) -> Result<Value, Error> {
    // TODO: this ideally understands which type of escaping is in use
    if v.is_safe() {
        Ok(v)
    } else {
        Ok(Value::from_safe_string(
            HtmlEscape(&v.to_string()).to_string(),
        ))
    }
}

#[cfg(feature = "builtins")]
mod builtins {
    use super::*;

    use crate::error::ErrorKind;
    use crate::utils::matches;
    use crate::value::{ValueKind, ValueRepr};
    use std::fmt::Write;
    use std::mem;

    /// Converts a value to uppercase.
    ///
    /// ```jinja
    /// <h1>{{ chapter.title|upper }}</h1>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn upper(_state: &State, v: String) -> Result<String, Error> {
        Ok(v.to_uppercase())
    }

    /// Converts a value to lowercase.
    ///
    /// ```jinja
    /// <h1>{{ chapter.title|lower }}</h1>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn lower(_state: &State, v: String) -> Result<String, Error> {
        Ok(v.to_lowercase())
    }

    /// Converts a value to title case.
    ///
    /// ```jinja
    /// <h1>{{ chapter.title|title }}</h1>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn title(_state: &State, v: String) -> Result<String, Error> {
        let mut rv = String::new();
        let mut capitalize = true;
        for c in v.chars() {
            if c.is_ascii_punctuation() || c.is_whitespace() {
                rv.push(c);
                capitalize = true;
            } else if capitalize {
                write!(rv, "{}", c.to_uppercase()).unwrap();
                capitalize = false;
            } else {
                write!(rv, "{}", c.to_lowercase()).unwrap();
            }
        }
        Ok(rv)
    }

    /// Does a string replace.
    ///
    /// It replaces all ocurrences of the first parameter with the second.
    ///
    /// ```jinja
    /// {{ "Hello World"|replace("Hello", "Goodbye") }}
    ///   -> Goodbye World
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn replace(_state: &State, v: String, from: String, to: String) -> Result<String, Error> {
        Ok(v.replace(&from, &to))
    }

    /// Returns the "length" of the value
    ///
    /// By default this filter is also registered under the alias `count`.
    ///
    /// ```jinja
    /// <p>Search results: {{ results|length }}
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn length(_state: &State, v: Value) -> Result<Value, Error> {
        v.len().map(Value::from).ok_or_else(|| {
            Error::new(
                ErrorKind::ImpossibleOperation,
                format!("cannot calculate length of value of type {}", v.kind()),
            )
        })
    }

    /// Dict sorting functionality.
    ///
    /// This filter works like `|items` but sorts the pairs by key first.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn dictsort(_state: &State, v: Value) -> Result<Value, Error> {
        let mut pairs = match v.0 {
            ValueRepr::Map(ref v) => v.iter().collect::<Vec<_>>(),
            _ => {
                return Err(Error::new(
                    ErrorKind::ImpossibleOperation,
                    "cannot convert value into pair list",
                ))
            }
        };
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        Ok(Value::from(
            pairs
                .into_iter()
                .map(|(k, v)| vec![Value::from(k.clone()), v.clone()])
                .collect::<Vec<_>>(),
        ))
    }

    /// Returns a list of pairs (items) from a mapping.
    ///
    /// This can be used to iterate over keys and values of a mapping
    /// at once.  Note that this will use the original order of the map
    /// which is typically arbitrary unless the `preserve_order` feature
    /// is used in which case the original order of the map is retained.
    /// It's generally better to use `|dictsort` which sorts the map by
    /// key before iterating.
    ///
    /// ```jinja
    /// <dl>
    /// {% for key, value in my_dict|items %}
    ///   <dt>{{ key }}
    ///   <dd>{{ value }}
    /// {% endfor %}
    /// </dl>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn items(_state: &State, v: Value) -> Result<Value, Error> {
        Ok(Value::from(
            match v.0 {
                ValueRepr::Map(ref v) => v.iter().collect::<Vec<_>>(),
                _ => {
                    return Err(Error::new(
                        ErrorKind::ImpossibleOperation,
                        "cannot convert value into pair list",
                    ))
                }
            }
            .into_iter()
            .map(|(k, v)| vec![Value::from(k.clone()), v.clone()])
            .collect::<Vec<_>>(),
        ))
    }

    /// Reverses a list or string
    ///
    /// ```jinja
    /// {% for user in users|reverse %}
    ///   <li>{{ user.name }}
    /// {% endfor %}
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn reverse(_state: &State, v: Value) -> Result<Value, Error> {
        if let Some(s) = v.as_str() {
            Ok(Value::from(s.chars().rev().collect::<String>()))
        } else if matches!(v.kind(), ValueKind::Seq) {
            let mut v = v.try_into_vec()?;
            v.reverse();
            Ok(Value::from(v))
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                format!("cannot reverse value of type {}", v.kind()),
            ))
        }
    }

    /// Trims a value
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn trim(_state: &State, s: String, chars: Option<String>) -> Result<String, Error> {
        match chars {
            Some(chars) => {
                let chars = chars.chars().collect::<Vec<_>>();
                Ok(s.trim_matches(&chars[..]).to_string())
            }
            None => Ok(s.trim().to_string()),
        }
    }

    /// Joins a sequence by a character
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn join(_state: &State, val: Value, joiner: Option<String>) -> Result<String, Error> {
        if val.is_undefined() || val.is_none() {
            return Ok(String::new());
        }

        let joiner = joiner.as_ref().map_or("", |x| x.as_str());

        if let Some(s) = val.as_str() {
            let mut rv = String::new();
            for c in s.chars() {
                if !rv.is_empty() {
                    rv.push_str(joiner);
                }
                rv.push(c);
            }
            Ok(rv)
        } else if matches!(val.kind(), ValueKind::Seq) {
            let mut rv = String::new();
            for item in val.try_into_vec()? {
                if !rv.is_empty() {
                    rv.push_str(joiner);
                }
                if let Some(s) = item.as_str() {
                    rv.push_str(s);
                } else {
                    write!(rv, "{}", item).ok();
                }
            }
            Ok(rv)
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                format!("cannot join value of type {}", val.kind()),
            ))
        }
    }

    /// If the value is undefined it will return the passed default value,
    /// otherwise the value of the variable:
    ///
    /// ```jinja
    /// <p>{{ my_variable|default("my_variable was not defined") }}</p>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn default(_: &State, value: Value, other: Option<Value>) -> Result<Value, Error> {
        Ok(if value.is_undefined() {
            other.unwrap_or_else(|| Value::from(""))
        } else {
            value
        })
    }

    /// Returns the absolute value of a number.
    ///
    /// ```jinja
    /// |a - b| = {{ (a - b)|abs }}
    ///   -> |2 - 4| = 2
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn abs(_: &State, value: Value) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::I64(x) => Ok(Value::from(x.abs())),
            ValueRepr::I128(x) => Ok(Value::from(x.abs())),
            ValueRepr::F64(x) => Ok(Value::from(x.abs())),
            _ => Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot round value",
            )),
        }
    }

    /// Round the number to a given precision.
    ///
    /// Round the number to a given precision. The first parameter specifies the
    /// precision (default is 0).
    ///
    /// ```jinja
    /// {{ 42.55|round }}
    ///   -> 43.0
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn round(_: &State, value: Value, precision: Option<i32>) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::I64(_) | ValueRepr::I128(_) => Ok(value),
            ValueRepr::F64(val) => {
                let x = 10f64.powi(precision.unwrap_or(0));
                Ok(Value::from((x * val).round() / x))
            }
            _ => Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot round value",
            )),
        }
    }

    /// Returns the first item from a list.
    ///
    /// If the list is empty `undefined` is returned.
    ///
    /// ```jinja
    /// <dl>
    ///   <dt>primary email
    ///   <dd>{{ user.email_addresses|first|default('no user') }}
    /// </dl>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn first(_: &State, value: Value) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::String(s) | ValueRepr::SafeString(s) => {
                Ok(s.chars().next().map_or(Value::UNDEFINED, Value::from))
            }
            ValueRepr::Seq(ref s) => Ok(s.first().cloned().unwrap_or(Value::UNDEFINED)),
            _ => Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot get first item from value",
            )),
        }
    }

    /// Returns the last item from a list.
    ///
    /// If the list is empty `undefined` is returned.
    ///
    /// ```jinja
    /// <h2>Most Recent Update</h2>
    /// {% with update = updates|last %}
    ///   <dl>
    ///     <dt>Location
    ///     <dd>{{ update.location }}
    ///     <dt>Status
    ///     <dd>{{ update.status }}
    ///   </dl>
    /// {% endwith %}
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn last(_: &State, value: Value) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::String(s) | ValueRepr::SafeString(s) => {
                Ok(s.chars().rev().next().map_or(Value::UNDEFINED, Value::from))
            }
            ValueRepr::Seq(ref s) => Ok(s.last().cloned().unwrap_or(Value::UNDEFINED)),
            _ => Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot get last item from value",
            )),
        }
    }

    /// Converts the input value into a list.
    ///
    /// If the value is already a list, then it's returned unchanged.
    /// Applied to a map this returns the list of keys, applied to a
    /// string this returns the characters.  If the value is undefined
    /// an empty list is returned.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn list(_: &State, value: Value) -> Result<Value, Error> {
        match &value.0 {
            ValueRepr::Undefined => Ok(Value::from(Vec::<Value>::new())),
            ValueRepr::String(ref s) | ValueRepr::SafeString(ref s) => {
                Ok(Value::from(s.chars().map(Value::from).collect::<Vec<_>>()))
            }
            ValueRepr::Seq(_) => Ok(value.clone()),
            ValueRepr::Map(ref m) => Ok(Value::from(
                m.iter()
                    .map(|x| Value::from(x.0.clone()))
                    .collect::<Vec<_>>(),
            )),
            _ => Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot convert value to list",
            )),
        }
    }

    /// Converts the value into a boolean value.
    ///
    /// This behaves the same as the if statement does with regards to
    /// handling of boolean values.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn bool(_: &State, value: Value) -> Result<bool, Error> {
        Ok(value.is_true())
    }

    /// Slice an iterable and return a list of lists containing
    /// those items.
    ///
    /// Useful if you want to create a div containing three ul tags that
    /// represent columns:
    ///
    /// ```jinja
    /// <div class="columnwrapper">
    /// {% for column in items|slice(3) %}
    ///   <ul class="column-{{ loop.index }}">
    ///   {% for item in column %}
    ///     <li>{{ item }}</li>
    ///   {% endfor %}
    ///   </ul>
    /// {% endfor %}
    /// </div>
    /// ```
    ///
    /// If you pass it a second argument it’s used to fill missing values on the
    /// last iteration.
    pub fn slice(
        _: &State,
        value: Value,
        count: usize,
        fill_with: Option<Value>,
    ) -> Result<Value, Error> {
        let items = value.iter().collect::<Vec<_>>();
        let len = items.len();
        let items_per_slice = len / count;
        let slices_with_extra = len % count;
        let mut offset = 0;
        let mut rv = Vec::new();

        for slice in 0..count {
            let start = offset + slice * items_per_slice;
            if slice < slices_with_extra {
                offset += 1;
            }
            let end = offset + (slice + 1) * items_per_slice;
            let tmp = &items[start..end];

            if let Some(ref filler) = fill_with {
                if slice >= slices_with_extra {
                    let mut tmp = tmp.to_vec();
                    tmp.push(filler.clone());
                    rv.push(Value::from(tmp));
                    continue;
                }
            }

            rv.push(Value::from(tmp.to_vec()));
        }

        Ok(Value::from(rv))
    }

    /// Batch items.
    ///
    /// This filter works pretty much like `slice` just the other way round. It
    /// returns a list of lists with the given number of items. If you provide a
    /// second parameter this is used to fill up missing items.
    ///
    /// ```jinja
    /// <table>
    ///   {% for row in items|batch(3, '&nbsp;') %}
    ///   <tr>
    ///   {% for column in row %}
    ///     <td>{{ column }}</td>
    ///   {% endfor %}
    ///   </tr>
    ///   {% endfor %}
    /// </table>
    /// ```
    pub fn batch(
        _: &State,
        value: Value,
        count: usize,
        fill_with: Option<Value>,
    ) -> Result<Value, Error> {
        let mut rv = Vec::new();
        let mut tmp = Vec::with_capacity(count);

        for item in value.iter() {
            if tmp.len() == count {
                rv.push(Value::from(mem::replace(
                    &mut tmp,
                    Vec::with_capacity(count),
                )));
            }
            tmp.push(item);
        }

        if !tmp.is_empty() {
            if let Some(filler) = fill_with {
                for _ in 0..count - tmp.len() {
                    tmp.push(filler.clone());
                }
            }
            rv.push(Value::from(tmp));
        }

        Ok(Value::from(rv))
    }

    /// Dumps a value to JSON.
    ///
    /// This filter is only available if the `json` feature is enabled.  The resulting
    /// value is safe to use in HTML as well as it will not contain any special HTML
    /// characters.  The optional parameter to the filter can be set to `true` to enable
    /// pretty printing.  Not that the `"` character is left unchanged as it's the
    /// JSON string delimiter.  If you want to pass JSON serialized this way into an
    /// HTTP attribute use single quoted HTML attributes:
    ///
    /// ```jinja
    /// <script>
    ///   const GLOBAL_CONFIG = {{ global_config|tojson }};
    /// </script>
    /// <a href="#" data-info='{{ json_object|tojson }}'>...</a>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(all(feature = "builtins", feature = "json"))))]
    #[cfg(feature = "json")]
    pub fn tojson(_: &State, value: Value, pretty: Option<bool>) -> Result<Value, Error> {
        if pretty.unwrap_or(false) {
            serde_json::to_string_pretty(&value)
        } else {
            serde_json::to_string(&value)
        }
        .map_err(|err| {
            Error::new(ErrorKind::ImpossibleOperation, "cannot serialize to JSON").with_source(err)
        })
        .map(|s| {
            let mut rv = String::with_capacity(s.len());
            for c in s.chars() {
                match c {
                    '<' => rv.push_str("\\u003c"),
                    '>' => rv.push_str("\\u003e"),
                    '&' => rv.push_str("\\u0026"),
                    '\'' => rv.push_str("\\u0027"),
                    _ => rv.push(c),
                }
            }
            Value::from_safe_string(rv)
        })
    }

    /// URL encodes a value.
    ///
    /// If given a map it encodes the parameters into a query set, otherwise it
    /// encodes the stringified value.  If the value is none or undefined, an
    /// empty string is returned.
    ///
    /// ```jinja
    /// <a href="/search?{{ {"q": "my search", "lang": "fr"}|urlencode }}">Search</a>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(all(feature = "builtins", feature = "urlencode"))))]
    #[cfg(feature = "urlencode")]
    pub fn urlencode(_: &State, value: Value) -> Result<String, Error> {
        const SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
            .remove(b'/')
            .remove(b'.')
            .remove(b'-')
            .remove(b'_')
            .add(b' ');
        match &value.0 {
            ValueRepr::None | ValueRepr::Undefined => Ok("".into()),
            ValueRepr::Bytes(b) => Ok(percent_encoding::percent_encode(b, SET).to_string()),
            ValueRepr::String(s) | ValueRepr::SafeString(s) => {
                Ok(percent_encoding::utf8_percent_encode(s, SET).to_string())
            }
            ValueRepr::Map(ref val) => {
                let mut rv = String::new();
                for (idx, (k, v)) in val.iter().enumerate() {
                    if idx > 0 {
                        rv.push('&');
                    }
                    write!(
                        rv,
                        "{}={}",
                        percent_encoding::utf8_percent_encode(&k.to_string(), SET),
                        percent_encoding::utf8_percent_encode(&v.to_string(), SET)
                    )
                    .unwrap();
                }
                Ok(rv)
            }
            _ => Ok(percent_encoding::utf8_percent_encode(&value.to_string(), SET).to_string()),
        }
    }

    #[test]
    fn test_basics() {
        fn test(_: &State, a: u32, b: u32) -> Result<u32, Error> {
            Ok(a + b)
        }

        let env = crate::Environment::new();
        let state = State {
            env: &env,
            ctx: crate::vm::Context::default(),
            auto_escape: crate::AutoEscape::None,
            current_block: None,
            name: "<unknown>",
        };
        let bx = BoxedFilter::new(test);
        assert_eq!(
            bx.apply_to(&state, Value::from(23), vec![Value::from(42)])
                .unwrap(),
            Value::from(65)
        );
    }

    #[test]
    fn test_optional_args() {
        fn add(_: &State, val: u32, a: u32, b: Option<u32>) -> Result<u32, Error> {
            let mut sum = val + a;
            if let Some(b) = b {
                sum += b;
            }
            Ok(sum)
        }

        let env = crate::Environment::new();
        let state = State {
            env: &env,
            ctx: crate::vm::Context::default(),
            auto_escape: crate::AutoEscape::None,
            current_block: None,
            name: "<unknown>",
        };
        let bx = BoxedFilter::new(add);
        assert_eq!(
            bx.apply_to(&state, Value::from(23), vec![Value::from(42)])
                .unwrap(),
            Value::from(65)
        );
        assert_eq!(
            bx.apply_to(
                &state,
                Value::from(23),
                vec![Value::from(42), Value::UNDEFINED]
            )
            .unwrap(),
            Value::from(65)
        );
        assert_eq!(
            bx.apply_to(
                &state,
                Value::from(23),
                vec![Value::from(42), Value::from(1)]
            )
            .unwrap(),
            Value::from(66)
        );
    }
}

#[cfg(feature = "builtins")]
pub use self::builtins::*;
