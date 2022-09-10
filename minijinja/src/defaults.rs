use std::collections::BTreeMap;
use std::fmt;

use crate::error::{Error, ErrorKind};
use crate::filters::{self, BoxedFilter};
use crate::output::Output;
use crate::tests::BoxedTest;
use crate::utils::{AutoEscape, HtmlEscape};
use crate::value::{Value, ValueKind};

fn write_with_html_escaping(out: &mut Output, value: &Value) -> fmt::Result {
    if matches!(
        value.kind(),
        ValueKind::Undefined | ValueKind::None | ValueKind::Bool | ValueKind::Number
    ) {
        write!(out, "{}", value)
    } else if let Some(s) = value.as_str() {
        write!(out, "{}", HtmlEscape(s))
    } else {
        write!(out, "{}", HtmlEscape(&value.to_string()))
    }
}

pub(crate) fn no_auto_escape(_: &str) -> AutoEscape {
    AutoEscape::None
}

/// The default logic for auto escaping based on file extension.
///
/// * [`Html`](AutoEscape::Html): `.html`, `.htm`, `.xml`
#[cfg_attr(
    feature = "json",
    doc = r" * [`Json`](AutoEscape::Json): `.json`, `.js`, `.yml`"
)]
/// * [`None`](AutoEscape::None): _all others_
pub fn default_auto_escape_callback(name: &str) -> AutoEscape {
    match name.rsplit('.').next() {
        Some("html") | Some("htm") | Some("xml") => AutoEscape::Html,
        #[cfg(feature = "json")]
        Some("json") | Some("js") | Some("yaml") | Some("yml") => AutoEscape::Json,
        _ => AutoEscape::None,
    }
}

/// The default formatter.
///
/// This formatter takes a value and directly writes it into the output format
/// while honoring the requested auto escape format of the output.  If the
/// value is already marked as safe, it's handled as if no auto escaping
/// was requested.
///
/// * [`Html`](AutoEscape::Html): performs HTML escaping
#[cfg_attr(
    feature = "json",
    doc = r" * [`Json`](AutoEscape::Json): serializes values to JSON"
)]
/// * [`None`](AutoEscape::None): no escaping
/// * [`Custom(..)`](AutoEscape::Custom): results in an error
pub fn escape_formatter(out: &mut Output, value: &Value) -> Result<(), Error> {
    match (value.is_safe(), out.auto_escape()) {
        // safe values do not get escaped
        (true, _) | (_, AutoEscape::None) => write!(out, "{}", value)?,
        (false, AutoEscape::Html) => write_with_html_escaping(out, value)?,
        #[cfg(feature = "json")]
        (false, AutoEscape::Json) => {
            let value = serde_json::to_string(&value).map_err(|err| {
                Error::new(ErrorKind::BadSerialization, "unable to format to JSON").with_source(err)
            })?;
            write!(out, "{}", value)?
        }
        (false, AutoEscape::Custom(name)) => {
            return Err(Error::new(
                ErrorKind::ImpossibleOperation,
                format!(
                    "Default formatter does not know how to format to custom format '{}'",
                    name
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn get_builtin_filters() -> BTreeMap<&'static str, filters::BoxedFilter> {
    let mut rv = BTreeMap::new();
    rv.insert("safe", BoxedFilter::new(filters::safe));
    rv.insert("escape", BoxedFilter::new(filters::escape));
    rv.insert("e", BoxedFilter::new(filters::escape));
    #[cfg(feature = "builtins")]
    {
        rv.insert("lower", BoxedFilter::new(filters::lower));
        rv.insert("upper", BoxedFilter::new(filters::upper));
        rv.insert("title", BoxedFilter::new(filters::title));
        rv.insert("replace", BoxedFilter::new(filters::replace));
        rv.insert("length", BoxedFilter::new(filters::length));
        rv.insert("count", BoxedFilter::new(filters::length));
        rv.insert("dictsort", BoxedFilter::new(filters::dictsort));
        rv.insert("items", BoxedFilter::new(filters::items));
        rv.insert("reverse", BoxedFilter::new(filters::reverse));
        rv.insert("trim", BoxedFilter::new(filters::trim));
        rv.insert("join", BoxedFilter::new(filters::join));
        rv.insert("default", BoxedFilter::new(filters::default));
        rv.insert("round", BoxedFilter::new(filters::round));
        rv.insert("abs", BoxedFilter::new(filters::abs));
        rv.insert("first", BoxedFilter::new(filters::first));
        rv.insert("last", BoxedFilter::new(filters::last));
        rv.insert("d", BoxedFilter::new(filters::default));
        rv.insert("list", BoxedFilter::new(filters::list));
        rv.insert("bool", BoxedFilter::new(filters::bool));
        rv.insert("batch", BoxedFilter::new(filters::batch));
        rv.insert("slice", BoxedFilter::new(filters::slice));
        #[cfg(feature = "json")]
        {
            rv.insert("tojson", BoxedFilter::new(filters::tojson));
        }
        #[cfg(feature = "urlencode")]
        {
            rv.insert("urlencode", BoxedFilter::new(filters::urlencode));
        }
    }

    rv
}

pub(crate) fn get_builtin_tests() -> BTreeMap<&'static str, BoxedTest> {
    #[allow(unused_mut)]
    let mut rv = BTreeMap::new();
    #[cfg(feature = "builtins")]
    {
        use crate::tests;
        rv.insert("odd", BoxedTest::new(tests::is_odd));
        rv.insert("even", BoxedTest::new(tests::is_even));
        rv.insert("undefined", BoxedTest::new(tests::is_undefined));
        rv.insert("defined", BoxedTest::new(tests::is_defined));
        rv.insert("number", BoxedTest::new(tests::is_number));
        rv.insert("string", BoxedTest::new(tests::is_string));
        rv.insert("sequence", BoxedTest::new(tests::is_sequence));
        rv.insert("mapping", BoxedTest::new(tests::is_mapping));
        rv.insert("startingwith", BoxedTest::new(tests::is_startingwith));
        rv.insert("endingwith", BoxedTest::new(tests::is_endingwith));
    }
    rv
}

pub(crate) fn get_globals() -> BTreeMap<&'static str, Value> {
    #[allow(unused_mut)]
    let mut rv = BTreeMap::new();
    #[cfg(feature = "builtins")]
    {
        use crate::functions::{self, BoxedFunction};
        rv.insert("range", BoxedFunction::new(functions::range).to_value());
        rv.insert("dict", BoxedFunction::new(functions::dict).to_value());
        rv.insert("debug", BoxedFunction::new(functions::debug).to_value());
    }

    rv
}
