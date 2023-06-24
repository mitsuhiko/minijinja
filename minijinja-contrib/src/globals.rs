#[allow(unused)]
use minijinja::value::Value;

/// Returns the current time in UTC as unix timestamp.
///
/// To format this timestamp, use the [`datetimeformat`](crate::filters::datetimeformat) filter.
#[cfg(feature = "datetime")]
#[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
pub fn now() -> Value {
    let now = time::OffsetDateTime::now_utc();
    Value::from(((now.unix_timestamp_nanos() / 1000) as f64) / 1_000_000.0)
}
