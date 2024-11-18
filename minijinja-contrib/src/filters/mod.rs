use std::convert::TryFrom;

use minijinja::value::Value;
use minijinja::{Error, ErrorKind};

#[cfg(feature = "datetime")]
mod datetime;

#[cfg(feature = "datetime")]
pub use self::datetime::*;

/// Returns a plural suffix if the value is not 1, '1', or an object of
/// length 1.
///
/// By default, the plural suffix is 's' and the singular suffix is
/// empty (''). You can specify a singular suffix as the first argument (or
/// `None`, for the default). You can specify a plural suffix as the second
/// argument (or `None`, for the default).
///
/// ```jinja
/// {{ users|length }} user{{ users|pluralize }}.
/// ```
///
/// ```jinja
/// {{ entities|length }} entit{{ entities|pluralize("y", "ies") }}.
/// ```
///
/// ```jinja
/// {{ platypuses|length }} platypus{{ platypuses|pluralize(None, "es") }}.
/// ```
pub fn pluralize(v: Value, singular: Option<Value>, plural: Option<Value>) -> Result<Value, Error> {
    let is_singular = match v.len() {
        Some(val) => val == 1,
        None => match i64::try_from(v.clone()) {
            Ok(val) => val == 1,
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!(
                        "Pluralize argument is not an integer, or a sequence / object with a \
                         length but of type {}",
                        v.kind()
                    ),
                ));
            }
        },
    };

    let (rv, default) = if is_singular {
        (singular.unwrap_or(Value::UNDEFINED), "")
    } else {
        (plural.unwrap_or(Value::UNDEFINED), "s")
    };

    if rv.is_undefined() || rv.is_none() {
        Ok(Value::from(default))
    } else {
        Ok(rv)
    }
}

/// Chooses a random element from a sequence or string.
///
/// The random number generated can be seeded with the `RAND_SEED`
/// global context variable.
///
/// ```jinja
/// {{ [1, 2, 3, 4]|random }}
/// ```
#[cfg(feature = "rand")]
#[cfg_attr(docsrs, doc(cfg(feature = "rand")))]
pub fn random(state: &minijinja::State, seq: Value) -> Result<Value, Error> {
    use crate::globals::get_rng;
    use minijinja::value::ValueKind;
    use rand::Rng;

    if matches!(seq.kind(), ValueKind::Seq | ValueKind::String) {
        let len = seq.len().unwrap_or(0);
        let idx = get_rng(state).gen_range(0..len);
        seq.get_item_by_index(idx)
    } else {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            "can only select random elements from sequences",
        ))
    }
}

/// Formats the value like a "human-readable" file size.
///
/// For example. 13 kB, 4.1 MB, 102 Bytes, etc.  Per default decimal prefixes are
/// used (Mega, Giga, etc.),  if the second parameter is set to true
/// the binary prefixes are used (Mebi, Gibi).
pub fn filesizeformat(value: f64, binary: Option<bool>) -> String {
    const BIN_PREFIXES: &[&str] = &["KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"];
    const SI_PREFIXES: &[&str] = &["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    let (prefixes, base) = if binary.unwrap_or(false) {
        (BIN_PREFIXES, 1024.0)
    } else {
        (SI_PREFIXES, 1000.0)
    };

    if value == 1.0 {
        return "1 Byte".into();
    }
    let (sign, value) = if value < 0.0 {
        ("-", -value)
    } else {
        ("", value)
    };

    if value < base {
        format!("{}{} Bytes", sign, value)
    } else {
        for (idx, prefix) in prefixes.iter().enumerate() {
            let unit = base.powf(idx as f64 + 2.0);
            if value < unit || idx == prefixes.len() - 1 {
                return format!("{}{:.1} {}", sign, base * value / unit, prefix);
            }
        }
        unreachable!();
    }
}
