use std::convert::TryFrom;

use minijinja::value::{Kwargs, Value, ValueKind};
use minijinja::{Error, ErrorKind, State};
use serde::de::value::SeqDeserializer;
use serde::de::IntoDeserializer;
use serde::Deserialize;
use time::{format_description, OffsetDateTime};
use time_tz::OffsetDateTimeExt;

fn handle_serde_error(err: serde::de::value::Error) -> Error {
    Error::new(ErrorKind::InvalidOperation, "not a valid date").with_source(err)
}

fn value_to_datetime(value: Value) -> Result<OffsetDateTime, Error> {
    if let Some(s) = value.as_str() {
        Ok(OffsetDateTime::deserialize(s.into_deserializer()).map_err(handle_serde_error)?)
    } else if let Ok(v) = f64::try_from(value.clone()) {
        OffsetDateTime::from_unix_timestamp_nanos((v * 1e9) as i128)
            .map_err(|_| Error::new(ErrorKind::InvalidOperation, "date out of range"))
    } else if value.kind() == ValueKind::Seq {
        let mut items = Vec::new();
        for item in value.try_iter()? {
            items.push(i64::try_from(item)?);
        }
        Ok(
            OffsetDateTime::deserialize(SeqDeserializer::new(items.into_iter()))
                .map_err(handle_serde_error)?,
        )
    } else {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "value is not a datetime",
        ))
    }
}

/// Formats a timestamp as date and time.
///
/// The value needs to be a unix timestamp, or a parsable string (via the [`time`] crate).
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
/// This filter currently uses the `time` crate to format dates and uses the format
/// string specification of that crate in version 2.  For more information read the
/// [Format description documentation](https://time-rs.github.io/book/api/format-description.html).
/// Additionally some special formats are supported:
///
/// * `short`: a short date and time format (`2023-06-24 16:37`)
/// * `medium`: a medium length date and time format (`Jun 24 2023 16:37`)
/// * `long`: a longer date and time format (`June 24 2023 16:37:22`)
/// * `full`: a full date and time format (`Saturday, June 24 2023 16:37:22`)
/// * `unix`: a unix timestamp in seconds only (`1687624642`)
/// * `iso`: date and time in iso format (`2023-06-24T16:37:22+00:00`)
///
/// This filter requires the `datetime` feature, the timezone support requires the `timezone`
/// feature.
#[cfg_attr(docsrs, doc(cfg(feature = "datetime")))]
pub fn datetimeformat(state: &State, value: Value, kwargs: Kwargs) -> Result<String, Error> {
    #[allow(unused_mut)]
    let mut datetime = value_to_datetime(value)?;
    let configured_format = state.lookup("DATETIME_FORMAT");

    #[cfg(feature = "timezone")]
    {
        apply_timezone(state, &kwargs, &mut datetime)?;
    }

    let format = kwargs.get::<Option<&str>>("format")?.unwrap_or_else(|| {
        configured_format
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("medium")
    });
    kwargs.assert_all_used()?;

    datetime
        .format(
            &format_description::parse_borrowed::<2>(match format {
                "short" => "[year]-[month]-[day] [hour]:[minute]",
                "medium" => "[month repr:short] [day padding:none] [year] [hour]:[minute]",
                "long" => "[month repr:long] [day padding:none] [year] [hour]:[minute]:[second]",
                "full" => "[weekday], [month repr:long] [day padding:none] [year] [hour]:[minute]:[second].[subsecond]",
                "iso" => {
                    "[year]-[month]-[day]T[hour]:[minute]:[second]+[offset_hour]:[offset_minute]"
                }
                "unix" => "[unix_timestamp]",
                other => other,
            })
            .map_err(|err| {
                Error::new(ErrorKind::InvalidOperation, "invalid format string").with_source(err)
            })?,
        )
        .map_err(|err| {
            Error::new(ErrorKind::InvalidOperation, "failed to format date").with_source(err)
        })
}

/// Formats a timestamp as time.
///
/// The value needs to be a unix timestamp, or a parsable string (via the [`time`] crate).
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
/// This filter currently uses the `time` crate to format dates and uses the format
/// string specification of that crate in version 2.  For more information read the
/// [Format description documentation](https://time-rs.github.io/book/api/format-description.html).
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
    #[allow(unused_mut)]
    let mut datetime = value_to_datetime(value)?;
    let configured_format = state.lookup("TIME_FORMAT");

    #[cfg(feature = "timezone")]
    {
        apply_timezone(state, &kwargs, &mut datetime)?;
    }

    let format = kwargs.get::<Option<&str>>("format")?.unwrap_or_else(|| {
        configured_format
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("medium")
    });
    kwargs.assert_all_used()?;

    datetime
        .format(
            &format_description::parse_borrowed::<2>(match format {
                "short" | "medium" => "[hour]:[minute]",
                "long" => "[hour]:[minute]:[second]",
                "full" => "[hour]:[minute]:[second].[subsecond]",
                "iso" => {
                    "[year]-[month]-[day]T[hour]:[minute]:[second]+[offset_hour]:[offset_minute]"
                }
                "unix" => "[unix_timestamp]",
                other => other,
            })
            .map_err(|err| {
                Error::new(ErrorKind::InvalidOperation, "invalid format string").with_source(err)
            })?,
        )
        .map_err(|err| {
            Error::new(ErrorKind::InvalidOperation, "failed to format date").with_source(err)
        })
}

/// Formats a timestamp as date.
///
/// The value needs to be a unix timestamp, or a parsable string (via the [`time`] crate).
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
/// This filter currently uses the `time` crate to format dates and uses the format
/// string specification of that crate in version 2.  For more information read the
/// [Format description documentation](https://time-rs.github.io/book/api/format-description.html).
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
    #[allow(unused_mut)]
    let mut datetime = value_to_datetime(value)?;
    let configured_format = state.lookup("DATE_FORMAT");

    #[cfg(feature = "timezone")]
    {
        apply_timezone(state, &kwargs, &mut datetime)?;
    }

    let format = kwargs.get::<Option<&str>>("format")?.unwrap_or_else(|| {
        configured_format
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("medium")
    });
    kwargs.assert_all_used()?;

    datetime
        .format(
            &format_description::parse_borrowed::<2>(match format {
                "short" => "[year]-[month]-[day]",
                "medium" => "[month repr:short] [day padding:none] [year]",
                "long" => "[month repr:long] [day padding:none] [year]",
                "full" => "[weekday], [month repr:long] [day padding:none] [year]",
                other => other,
            })
            .map_err(|err| {
                Error::new(ErrorKind::InvalidOperation, "invalid format string").with_source(err)
            })?,
        )
        .map_err(|err| {
            Error::new(ErrorKind::InvalidOperation, "failed to format date").with_source(err)
        })
}

fn apply_timezone(
    state: &State,
    kwargs: &Kwargs,
    datetime: &mut OffsetDateTime,
) -> Result<(), Error> {
    let configured_tz = state.lookup("TIMEZONE");
    let tzname = kwargs.get::<Option<&str>>("tz")?.unwrap_or_else(|| {
        configured_tz
            .as_ref()
            .and_then(|x| x.as_str())
            .unwrap_or("original")
    });
    if tzname != "original" {
        let tz = time_tz::timezones::get_by_name(tzname).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("unknown timezone '{}'", tzname),
            )
        })?;
        *datetime = datetime.to_timezone(tz)
    };
    Ok(())
}
