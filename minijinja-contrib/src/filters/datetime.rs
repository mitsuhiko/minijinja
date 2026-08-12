use std::convert::TryFrom;

use jiff::civil::{Date, DateTime, Time};
use jiff::fmt::{strtime, temporal::Pieces};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use minijinja::value::{Kwargs, Value};
use minijinja::{Error, ErrorKind, State};

enum ParsedDateTime {
    Date(Date),
    Civil(DateTime),
    Zoned(Zoned),
}

fn invalid_datetime(err: jiff::Error) -> Error {
    Error::new(ErrorKind::InvalidOperation, "not a valid date or timestamp").with_source(err)
}

fn date_out_of_range(err: jiff::Error) -> Error {
    Error::new(ErrorKind::InvalidOperation, "date out of range").with_source(err)
}

fn parse_string(value: &str) -> Result<ParsedDateTime, Error> {
    let pieces = Pieces::parse(value).map_err(invalid_datetime)?;
    let Some(time) = pieces.time() else {
        return Ok(ParsedDateTime::Date(pieces.date()));
    };
    let datetime = pieces.date().to_datetime(time);

    #[cfg(feature = "timezone")]
    let annotated_timezone = pieces.to_time_zone().map_err(invalid_datetime)?;
    #[cfg(not(feature = "timezone"))]
    let annotated_timezone: Option<TimeZone> = None;

    if let Some(offset) = pieces.to_numeric_offset() {
        let timestamp = offset.to_timestamp(datetime).map_err(date_out_of_range)?;
        let timezone = annotated_timezone.unwrap_or_else(|| TimeZone::fixed(offset));
        Ok(ParsedDateTime::Zoned(timestamp.to_zoned(timezone)))
    } else if let Some(timezone) = annotated_timezone {
        Ok(ParsedDateTime::Zoned(
            datetime.to_zoned(timezone).map_err(date_out_of_range)?,
        ))
    } else {
        Ok(ParsedDateTime::Civil(datetime))
    }
}

fn value_to_datetime(
    value: Value,
    state: &State,
    kwargs: &Kwargs,
    allow_date: bool,
) -> Result<Zoned, Error> {
    let parsed = if let Some(value) = value.as_str() {
        parse_string(value)?
    } else if let Ok(value) = f64::try_from(value.clone()) {
        let timestamp =
            Timestamp::from_nanosecond((value * 1e9) as i128).map_err(date_out_of_range)?;
        ParsedDateTime::Zoned(timestamp.to_zoned(TimeZone::UTC))
    } else {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "value is not a datetime",
        ));
    };

    let datetime = match parsed {
        ParsedDateTime::Date(date) => {
            if !allow_date {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "filter requires time, but only received a date",
                ));
            }
            date.to_datetime(Time::midnight())
                .to_zoned(TimeZone::UTC)
                .map_err(date_out_of_range)?
        }
        ParsedDateTime::Civil(datetime) => {
            #[cfg(feature = "timezone")]
            let timezone = get_timezone(state, kwargs)?.unwrap_or(TimeZone::UTC);
            #[cfg(not(feature = "timezone"))]
            let timezone = {
                let _ = (state, kwargs);
                TimeZone::UTC
            };
            datetime.to_zoned(timezone).map_err(date_out_of_range)?
        }
        ParsedDateTime::Zoned(datetime) => {
            #[cfg(feature = "timezone")]
            if let Some(timezone) = get_timezone(state, kwargs)? {
                return Ok(datetime.with_time_zone(timezone));
            }
            datetime
        }
    };
    Ok(datetime)
}

#[cfg(feature = "timezone")]
fn get_timezone(state: &State<'_, '_>, kwargs: &Kwargs) -> Result<Option<TimeZone>, Error> {
    let configured_tz = state.lookup("TIMEZONE");
    let tzname = kwargs.get::<Option<&str>>("tz")?.unwrap_or_else(|| {
        configured_tz
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("original")
    });
    if tzname == "original" {
        Ok(None)
    } else {
        TimeZone::get(tzname).map(Some).map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("unknown timezone '{tzname}'"),
            )
        })
    }
}

fn format_datetime(datetime: &Zoned, format: &str) -> Result<String, Error> {
    strtime::format(format, datetime).map_err(|err| {
        Error::new(ErrorKind::InvalidOperation, "invalid format string").with_source(err)
    })
}

/// Formats a timestamp as date and time.
///
/// The value needs to be a Unix timestamp, an ISO 8601 string, or a serialized
/// `chrono` or Jiff date/time value.
///
/// The filter accepts two keyword arguments (`format` and `tz`) to influence the format
/// and the timezone.  The default format is `"medium"`.  The defaults for these keyword
/// arguments are taken from two global variables in the template context: `DATETIME_FORMAT`
/// and `TIMEZONE`.  If the timezone is set to `"original"` or is not configured, then
/// the timezone of the value is retained.  Otherwise the timezone is the name of a
/// timezone [from the database](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones).
///
/// ```jinja
/// {{ value|datetimeformat }}
/// ```
///
/// ```jinja
/// {{ value|datetimeformat(format="short") }}
/// ```
///
/// ```jinja
/// {{ value|datetimeformat(format="short", tz="Europe/Vienna") }}
/// ```
///
/// This filter uses Jiff and accepts `strftime`-style format strings. For more
/// information, see Jiff's [`strtime` documentation](https://docs.rs/jiff/latest/jiff/fmt/strtime/).
/// Additionally some special formats are supported:
///
/// * `short`: a short date and time format (`2023-06-24 16:37`)
/// * `medium`: a medium length date and time format (`Jun 24 2023 16:37`)
/// * `long`: a longer date and time format (`June 24 2023 16:37:22`)
/// * `full`: a full date and time format (`Saturday, June 24 2023 16:37:22.0`)
/// * `unix`: a unix timestamp in seconds only (`1687624642`)
/// * `iso`: date and time in iso format (`2023-06-24T16:37:22+00:00`)
///
/// This filter requires the `datetime` feature, the timezone support requires the `timezone`
/// feature.
#[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
pub fn datetimeformat(state: &State, value: Value, kwargs: Kwargs) -> Result<String, Error> {
    let datetime = value_to_datetime(value, state, &kwargs, false)?;
    let configured_format = state.lookup("DATETIME_FORMAT");

    let format = kwargs.get::<Option<&str>>("format")?.unwrap_or_else(|| {
        configured_format
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("medium")
    });
    kwargs.assert_all_used()?;

    format_datetime(
        &datetime,
        match format {
            "short" => "%Y-%m-%d %H:%M",
            "medium" => "%b %-d %Y %H:%M",
            "long" => "%B %-d %Y %H:%M:%S",
            "full" => "%A, %B %-d %Y %H:%M:%S.%f",
            "iso" => "%Y-%m-%dT%H:%M:%S%:z",
            "unix" => "%s",
            other => other,
        },
    )
}

/// Formats a timestamp as time.
///
/// The value needs to be a Unix timestamp, an ISO 8601 string, or a serialized
/// `chrono` or Jiff date/time value.
///
/// The filter accepts two keyword arguments (`format` and `tz`) to influence the format
/// and the timezone.  The default format is `"medium"`.  The defaults for these keyword
/// arguments are taken from two global variables in the template context: `TIME_FORMAT`
/// and `TIMEZONE`.  If the timezone is set to `"original"` or is not configured, then
/// the timezone of the value is retained.  Otherwise the timezone is the name of a
/// timezone [from the database](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones).
///
/// ```jinja
/// {{ value|timeformat }}
/// ```
///
/// ```jinja
/// {{ value|timeformat(format="short") }}
/// ```
///
/// ```jinja
/// {{ value|timeformat(format="short", tz="Europe/Vienna") }}
/// ```
///
/// This filter uses Jiff and accepts `strftime`-style format strings. For more
/// information, see Jiff's [`strtime` documentation](https://docs.rs/jiff/latest/jiff/fmt/strtime/).
/// Additionally some special formats are supported:
///
/// * `short` and `medium`: hour and minute (`16:37`)
/// * `long`: includes seconds too (`16:37:22`)
/// * `full`: includes subseconds too (`16:37:22.0`)
/// * `unix`: a unix timestamp in seconds only (`1687624642`)
/// * `iso`: date and time in iso format (`2023-06-24T16:37:22+00:00`)
///
/// This filter requires the `datetime` feature, the timezone support requires the `timezone`
/// feature.
#[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
pub fn timeformat(state: &State, value: Value, kwargs: Kwargs) -> Result<String, Error> {
    let datetime = value_to_datetime(value, state, &kwargs, false)?;
    let configured_format = state.lookup("TIME_FORMAT");

    let format = kwargs.get::<Option<&str>>("format")?.unwrap_or_else(|| {
        configured_format
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("medium")
    });
    kwargs.assert_all_used()?;

    format_datetime(
        &datetime,
        match format {
            "short" | "medium" => "%H:%M",
            "long" => "%H:%M:%S",
            "full" => "%H:%M:%S.%f",
            "iso" => "%Y-%m-%dT%H:%M:%S%:z",
            "unix" => "%s",
            other => other,
        },
    )
}

/// Formats a timestamp as date.
///
/// The value needs to be a Unix timestamp, an ISO 8601 string, or a serialized
/// `chrono` or Jiff date/time value. If the string does not include time
/// information, then timezone adjustments are not performed.
///
/// The filter accepts two keyword arguments (`format` and `tz`) to influence the format
/// and the timezone.  The default format is `"medium"`.  The defaults for these keyword
/// arguments are taken from two global variables in the template context: `DATE_FORMAT`
/// and `TIMEZONE`.  If the timezone is set to `"original"` or is not configured, then
/// the timezone of the value is retained.  Otherwise the timezone is the name of a
/// timezone [from the database](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones).
///
/// ```jinja
/// {{ value|dateformat }}
/// ```
///
/// ```jinja
/// {{ value|dateformat(format="short") }}
/// ```
///
/// ```jinja
/// {{ value|dateformat(format="short", tz="Europe/Vienna") }}
/// ```
///
/// This filter uses Jiff and accepts `strftime`-style format strings. For more
/// information, see Jiff's [`strtime` documentation](https://docs.rs/jiff/latest/jiff/fmt/strtime/).
/// Additionally some special formats are supported:
///
/// * `short`: a short date format (`2023-06-24`)
/// * `medium`: a medium length date format (`Jun 24 2023`)
/// * `long`: a longer date format (`June 24 2023`)
/// * `full`: a full date format (`Saturday, June 24 2023`)
///
/// This filter requires the `datetime` feature, the timezone support requires the `timezone`
/// feature.
#[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
pub fn dateformat(state: &State, value: Value, kwargs: Kwargs) -> Result<String, Error> {
    let datetime = value_to_datetime(value, state, &kwargs, true)?;
    let configured_format = state.lookup("DATE_FORMAT");

    let format = kwargs.get::<Option<&str>>("format")?.unwrap_or_else(|| {
        configured_format
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("medium")
    });
    kwargs.assert_all_used()?;

    format_datetime(
        &datetime,
        match format {
            "short" => "%Y-%m-%d",
            "medium" => "%b %-d %Y",
            "long" => "%B %-d %Y",
            "full" => "%A, %B %-d %Y",
            other => other,
        },
    )
}
