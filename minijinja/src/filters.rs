//! Filter functions and abstractions.
//!
//! MiniJinja inherits from Jinja2 the concept of filter functions.  These are functions
//! which are applied to values to modify them.  For example the expression `{{ 42|filter(23) }}`
//! invokes the filter `filter` with the arguments `42` and `23`.
//!
//! MiniJinja comes with some built-in filters that are listed below. To create a
//! custom filter write a function that takes at least a value, then registers it
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
//! A custom filter is just a simple function which accepts its inputs
//! as parameters and then returns a new value.  For instance the following
//! shows a filter which takes an input value and replaces whitespace with
//! dashes and converts it to lowercase:
//!
//! ```
//! # use minijinja::Environment;
//! # let mut env = Environment::new();
//! fn slugify(value: String) -> String {
//!     value.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-")
//! }
//!
//! env.add_filter("slugify", slugify);
//! ```
//!
//! MiniJinja will perform the necessary conversions automatically.  For more
//! information see the [`Filter`] trait.
//!
//! # Accessing State
//!
//! In some cases it can be necesary to access the execution [`State`].  Since a borrowed
//! state implements [`ArgType`](crate::value::ArgType) it's possible to add a
//! parameter that holds the state.  For instance the following filter appends
//! the current template name to the string:
//!
//! ```
//! # use minijinja::Environment;
//! # let mut env = Environment::new();
//! use minijinja::value::Value;
//! use minijinja::State;
//!
//! fn append_template(state: &State, value: &Value) -> String {
//!     format!("{}-{}", value, state.name())
//! }
//!
//! env.add_filter("appendTemplate", append_template);
//! ```
//!
//! # Built-in Filters
//!
//! When the `builtins` feature is enabled a range of built-in filters are
//! automatically added to the environment.  These are also all provided in
//! this module.  Note though that these functions are not to be
//! called from Rust code as their exact interface (arguments and return types)
//! might change from one MiniJinja version to another.
use std::sync::Arc;

use crate::error::Error;
use crate::utils::{write_escaped, SealedMarker};
use crate::value::{ArgType, FunctionArgs, FunctionResult, Value};
use crate::vm::State;
use crate::{AutoEscape, Output};

type FilterFunc = dyn Fn(&State, &[Value]) -> Result<Value, Error> + Sync + Send + 'static;

#[derive(Clone)]
pub(crate) struct BoxedFilter(Arc<FilterFunc>);

/// A utility trait that represents filters.
///
/// This trait is used by the [`add_filter`](crate::Environment::add_filter) method to abstract over
/// different types of functions that implement filters.  Filters are functions
/// which at the very least accept the [`State`] by reference as first parameter
/// and the value that that the filter is applied to as second.  Additionally up to
/// 4 further parameters are supported.
///
/// A filter can return any of the following types:
///
/// * `Rv` where `Rv` implements `Into<Value>`
/// * `Result<Rv, Error>` where `Rv` implements `Into<Value>`
///
/// Filters accept one mandatory parameter which is the value the filter is
/// applied to and up to 4 extra parameters.  The extra parameters can be
/// marked optional by using `Option<T>`.  The last argument can also use
/// [`Rest<T>`](crate::value::Rest) to capture the remaining arguments.  All
/// types are supported for which [`ArgType`](crate::value::ArgType) is implemented.
///
/// For a list of built-in filters see [`filters`](crate::filters).
///
/// # Basic Example
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::State;
///
/// fn slugify(value: String) -> String {
///     value.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-")
/// }
///
/// env.add_filter("slugify", slugify);
/// ```
///
/// ```jinja
/// {{ "Foo Bar Baz"|slugify }} -> foo-bar-baz
/// ```
///
/// # Arguments and Optional Arguments
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// fn substr(value: String, start: u32, end: Option<u32>) -> String {
///     let end = end.unwrap_or(value.len() as _);
///     value.get(start as usize..end as usize).unwrap_or_default().into()
/// }
///
/// env.add_filter("substr", substr);
/// ```
///
/// ```jinja
/// {{ "Foo Bar Baz"|substr(4) }} -> Bar Baz
/// {{ "Foo Bar Baz"|substr(4, 7) }} -> Bar
/// ```
///
/// # Variadic
///
/// ```
/// # use minijinja::Environment;
/// # let mut env = Environment::new();
/// use minijinja::value::Rest;
///
/// fn pyjoin(joiner: String, values: Rest<String>) -> String {
///     values.connect(&joiner)
/// }
///
/// env.add_filter("pyjoin", pyjoin);
/// ```
///
/// ```jinja
/// {{ "|".join(1, 2, 3) }} -> 1|2|3
/// ```
pub trait Filter<Rv, Args>: Send + Sync + 'static {
    /// Applies a filter to value with the given arguments.
    ///
    /// The value is always the first argument.
    #[doc(hidden)]
    fn apply_to(&self, args: Args, _: SealedMarker) -> Rv;
}

macro_rules! tuple_impls {
    ( $( $name:ident )* ) => {
        impl<Func, Rv, $($name),*> Filter<Rv, ($($name,)*)> for Func
        where
            Func: Fn($($name),*) -> Rv + Send + Sync + 'static,
            Rv: FunctionResult,
            $($name: for<'a> ArgType<'a>,)*
        {
            fn apply_to(&self, args: ($($name,)*), _: SealedMarker) -> Rv {
                #[allow(non_snake_case)]
                let ($($name,)*) = args;
                (self)($($name,)*)
            }
        }
    };
}

tuple_impls! {}
tuple_impls! { A }
tuple_impls! { A B }
tuple_impls! { A B C }
tuple_impls! { A B C D }
tuple_impls! { A B C D E }

impl BoxedFilter {
    /// Creates a new boxed filter.
    pub fn new<F, Rv, Args>(f: F) -> BoxedFilter
    where
        F: Filter<Rv, Args> + for<'a> Filter<Rv, <Args as FunctionArgs<'a>>::Output>,
        Rv: FunctionResult,
        Args: for<'a> FunctionArgs<'a>,
    {
        BoxedFilter(Arc::new(move |state, args| -> Result<Value, Error> {
            f.apply_to(ok!(Args::from_values(Some(state), args)), SealedMarker)
                .into_result()
        }))
    }

    /// Applies the filter to a value and argument.
    pub fn apply_to(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        (self.0)(state, args)
    }
}

/// Marks a value as safe.  This converts it into a string.
///
/// When a value is marked as safe, no further auto escaping will take place.
pub fn safe(v: String) -> Value {
    Value::from_safe_string(v)
}

/// Escapes a string.  By default to HTML.
///
/// By default this filter is also registered under the alias `e`.  Note that
/// this filter escapes with the format that is native to the format or HTML
/// otherwise.  This means that if the auto escape setting is set to
/// `Json` for instance then this filter will serialize to JSON instead.
pub fn escape(state: &State, v: Value) -> Result<Value, Error> {
    if v.is_safe() {
        return Ok(v);
    }

    // this tries to use the escaping flag of the current scope, then
    // of the initial state and if that is also not set it falls back
    // to HTML.
    let auto_escape = match state.auto_escape() {
        AutoEscape::None => match state.env().get_initial_auto_escape(state.name()) {
            AutoEscape::None => AutoEscape::Html,
            other => other,
        },
        other => other,
    };
    let mut rv = String::new();
    let mut out = Output::with_string(&mut rv);
    ok!(write_escaped(&mut out, auto_escape, &v));
    Ok(Value::from_safe_string(rv))
}

#[cfg(feature = "builtins")]
mod builtins {
    use super::*;

    use crate::error::ErrorKind;
    use crate::value::{ValueKind, ValueRepr};
    use std::borrow::Cow;
    use std::fmt::Write;
    use std::mem;

    #[cfg(test)]
    use similar_asserts::assert_eq;

    /// Converts a value to uppercase.
    ///
    /// ```jinja
    /// <h1>{{ chapter.title|upper }}</h1>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn upper(v: Cow<'_, str>) -> String {
        v.to_uppercase()
    }

    /// Converts a value to lowercase.
    ///
    /// ```jinja
    /// <h1>{{ chapter.title|lower }}</h1>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn lower(v: Cow<'_, str>) -> String {
        v.to_lowercase()
    }

    /// Converts a value to title case.
    ///
    /// ```jinja
    /// <h1>{{ chapter.title|title }}</h1>
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn title(v: Cow<'_, str>) -> String {
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
        rv
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
    pub fn replace(
        _state: &State,
        v: Cow<'_, str>,
        from: Cow<'_, str>,
        to: Cow<'_, str>,
    ) -> String {
        v.replace(&from as &str, &to as &str)
    }

    /// Returns the "length" of the value
    ///
    /// By default this filter is also registered under the alias `count`.
    ///
    /// ```jinja
    /// <p>Search results: {{ results|length }}
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn length(v: Value) -> Result<usize, Error> {
        v.len().ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("cannot calculate length of value of type {}", v.kind()),
            )
        })
    }

    /// Dict sorting functionality.
    ///
    /// This filter works like `|items` but sorts the pairs by key first.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn dictsort(v: Value) -> Result<Value, Error> {
        let mut pairs = match v.0 {
            ValueRepr::Map(ref v, _) => v.iter().collect::<Vec<_>>(),
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
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
    pub fn items(v: Value) -> Result<Value, Error> {
        Ok(Value::from(
            match v.0 {
                ValueRepr::Map(ref v, _) => v.iter(),
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        "cannot convert value into pair list",
                    ))
                }
            }
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
    pub fn reverse(v: Value) -> Result<Value, Error> {
        if let Some(s) = v.as_str() {
            Ok(Value::from(s.chars().rev().collect::<String>()))
        } else if matches!(v.kind(), ValueKind::Seq) {
            Ok(Value::from(
                ok!(v.as_slice()).iter().rev().cloned().collect::<Vec<_>>(),
            ))
        } else {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("cannot reverse value of type {}", v.kind()),
            ))
        }
    }

    /// Trims a value
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn trim(s: Cow<'_, str>, chars: Option<Cow<'_, str>>) -> String {
        match chars {
            Some(chars) => {
                let chars = chars.chars().collect::<Vec<_>>();
                s.trim_matches(&chars[..]).to_string()
            }
            None => s.trim().to_string(),
        }
    }

    /// Joins a sequence by a character
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn join(val: Value, joiner: Option<Cow<'_, str>>) -> Result<String, Error> {
        if val.is_undefined() || val.is_none() {
            return Ok(String::new());
        }

        let joiner = joiner.as_ref().unwrap_or(&Cow::Borrowed(""));

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
            for item in ok!(val.as_slice()) {
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
                ErrorKind::InvalidOperation,
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
    pub fn default(value: Value, other: Option<Value>) -> Value {
        if value.is_undefined() {
            other.unwrap_or_else(|| Value::from(""))
        } else {
            value
        }
    }

    /// Returns the absolute value of a number.
    ///
    /// ```jinja
    /// |a - b| = {{ (a - b)|abs }}
    ///   -> |2 - 4| = 2
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn abs(value: Value) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::I64(x) => Ok(Value::from(x.abs())),
            ValueRepr::I128(x) => Ok(Value::from(x.0.abs())),
            ValueRepr::F64(x) => Ok(Value::from(x.abs())),
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
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
    pub fn round(value: Value, precision: Option<i32>) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::I64(_) | ValueRepr::I128(_) => Ok(value),
            ValueRepr::F64(val) => {
                let x = 10f64.powi(precision.unwrap_or(0));
                Ok(Value::from((x * val).round() / x))
            }
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
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
    pub fn first(value: Value) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::String(s, _) => Ok(s.chars().next().map_or(Value::UNDEFINED, Value::from)),
            ValueRepr::Seq(ref s) => Ok(s.first().cloned().unwrap_or(Value::UNDEFINED)),
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
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
    pub fn last(value: Value) -> Result<Value, Error> {
        match value.0 {
            ValueRepr::String(s, _) => {
                Ok(s.chars().rev().next().map_or(Value::UNDEFINED, Value::from))
            }
            ValueRepr::Seq(ref s) => Ok(s.last().cloned().unwrap_or(Value::UNDEFINED)),
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
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
    pub fn list(value: Value) -> Result<Value, Error> {
        match &value.0 {
            ValueRepr::Undefined => Ok(Value::from(Vec::<Value>::new())),
            ValueRepr::String(ref s, _) => {
                Ok(Value::from(s.chars().map(Value::from).collect::<Vec<_>>()))
            }
            ValueRepr::Seq(_) => Ok(value.clone()),
            ValueRepr::Map(ref m, _) => Ok(Value::from(
                m.iter()
                    .map(|x| Value::from(x.0.clone()))
                    .collect::<Vec<_>>(),
            )),
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
                "cannot convert value to list",
            )),
        }
    }

    /// Converts the value into a boolean value.
    ///
    /// This behaves the same as the if statement does with regards to
    /// handling of boolean values.
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn bool(value: Value) -> bool {
        value.is_true()
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
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn slice(value: Value, count: usize, fill_with: Option<Value>) -> Result<Value, Error> {
        if count == 0 {
            return Err(Error::new(ErrorKind::InvalidOperation, "count cannot be 0"));
        }
        let items = ok!(value.try_iter_owned()).collect::<Vec<_>>();
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
    #[cfg_attr(docsrs, doc(cfg(feature = "builtins")))]
    pub fn batch(value: Value, count: usize, fill_with: Option<Value>) -> Result<Value, Error> {
        let mut rv = Vec::new();
        let mut tmp = Vec::with_capacity(count);

        for item in ok!(value.try_iter_owned()) {
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
    pub fn tojson(value: Value, pretty: Option<bool>) -> Result<Value, Error> {
        if pretty.unwrap_or(false) {
            serde_json::to_string_pretty(&value)
        } else {
            serde_json::to_string(&value)
        }
        .map_err(|err| {
            Error::new(ErrorKind::InvalidOperation, "cannot serialize to JSON").with_source(err)
        })
        .map(|s| {
            // When this filter is used the return value is safe for both HTML and JSON
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

    /// indents Value with spaces or tabs
    ///
    /// This filter is only available if the `indent` feature is enabled.  The resulting
    /// characters.  The optional parameter to the filter can be set to `true` to enable 
    /// indenting with \t. 
    /// This filter is useful, if you want to template yaml-files
    ///
    /// ```jinja
    /// example:
    ///   config:
    /// {{ global_config|indent(2,true) }}; #indent with 2 Tabs
    /// {{ glabal_config|indent(4) }} #indent with 4 
    /// ```
    #[cfg_attr(docsrs, doc(cfg(all(feature = "builtins", feature = "indent"))))]
    #[cfg(feature = "indent")]
    pub fn indent(value: String, spaces: usize, tabs: Option<bool>) -> String {
        let mut output: String = String::new();
        if tabs.unwrap_or(false) {
            for line in value.split('\n') {
                output.push_str(format!("{}{}\n", String::from("\t").repeat(spaces), line).as_str());
            }
        } else {
            for line in value.split('\n') {
                output.push_str(format!("{}{}\n", String::from(" ").repeat(spaces), line).as_str());
            }
        }
        output
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
    pub fn urlencode(value: Value) -> Result<String, Error> {
        const SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
            .remove(b'/')
            .remove(b'.')
            .remove(b'-')
            .remove(b'_')
            .add(b' ');
        match &value.0 {
            ValueRepr::None | ValueRepr::Undefined => Ok("".into()),
            ValueRepr::Bytes(b) => Ok(percent_encoding::percent_encode(b, SET).to_string()),
            ValueRepr::String(s, _) => {
                Ok(percent_encoding::utf8_percent_encode(s, SET).to_string())
            }
            ValueRepr::Map(ref val, _) => {
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
        fn test(a: u32, b: u32) -> Result<u32, Error> {
            Ok(a + b)
        }

        let env = crate::Environment::new();
        State::with_dummy(&env, |state| {
            let bx = BoxedFilter::new(test);
            assert_eq!(
                bx.apply_to(state, &[Value::from(23), Value::from(42)][..])
                    .unwrap(),
                Value::from(65)
            );
        });
    }

    #[test]
    fn test_rest_args() {
        fn sum(val: u32, rest: crate::value::Rest<u32>) -> u32 {
            rest.iter().fold(val, |a, b| a + b)
        }

        let env = crate::Environment::new();
        State::with_dummy(&env, |state| {
            let bx = BoxedFilter::new(sum);
            assert_eq!(
                bx.apply_to(
                    state,
                    &[
                        Value::from(1),
                        Value::from(2),
                        Value::from(3),
                        Value::from(4)
                    ][..]
                )
                .unwrap(),
                Value::from(1 + 2 + 3 + 4)
            );
        });
    }

    #[test]
    fn test_optional_args() {
        fn add(val: u32, a: u32, b: Option<u32>) -> Result<u32, Error> {
            // ensure we really get our value as first argument
            assert_eq!(val, 23);
            let mut sum = val + a;
            if let Some(b) = b {
                sum += b;
            }
            Ok(sum)
        }

        let env = crate::Environment::new();
        State::with_dummy(&env, |state| {
            let bx = BoxedFilter::new(add);
            assert_eq!(
                bx.apply_to(state, &[Value::from(23), Value::from(42)][..])
                    .unwrap(),
                Value::from(65)
            );
            assert_eq!(
                bx.apply_to(
                    state,
                    &[Value::from(23), Value::from(42), Value::UNDEFINED][..]
                )
                .unwrap(),
                Value::from(65)
            );
            assert_eq!(
                bx.apply_to(
                    state,
                    &[Value::from(23), Value::from(42), Value::from(1)][..]
                )
                .unwrap(),
                Value::from(66)
            );
        });
    }
}

#[cfg(feature = "builtins")]
pub use self::builtins::*;
