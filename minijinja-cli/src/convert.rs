//! Conversions between deser values and MiniJinja values.
//!
//! The data formats are parsed into [`deser_value::Value`]s which are then
//! converted into MiniJinja values.  For output (for instance `--expr-out
//! json`) MiniJinja values are converted back.
use deser::{Atom, ImplicitValue};
use deser_value::{Kind, Map};
use minijinja::value::{Value, ValueKind};

/// Converts a deser value into a MiniJinja value.
pub fn to_minijinja(value: &deser_value::Value) -> Value {
    match value.kind() {
        Kind::Null => Value::from(()),
        Kind::Bool(value) => Value::from(*value),
        Kind::U64(value) => Value::from(*value),
        Kind::I64(value) => Value::from(*value),
        Kind::F32(value) => Value::from(f64::from(*value)),
        Kind::F64(value) => Value::from(*value),
        Kind::Char(value) => Value::from(*value),
        Kind::Str(value) | Kind::Lexical(value) => Value::from(value.as_str()),
        Kind::Bytes(value) => Value::from_bytes(value.data().to_vec()),
        Kind::Implicit(value) => implicit_to_minijinja(value.value()),
        Kind::Ext(ext) => {
            // large integers are extension values, everything else (like
            // date-times) is represented by its fallback.
            if let Some(value) = ext.downcast_ref::<i128>() {
                Value::from(*value)
            } else if let Some(value) = ext.downcast_ref::<u128>() {
                Value::from(*value)
            } else {
                atom_to_minijinja(ext.fallback())
            }
        }
        Kind::Seq(seq) => seq.iter().map(to_minijinja).collect(),
        Kind::Map(map) => {
            Value::from_pairs(map.iter().map(|(k, v)| (to_minijinja(k), to_minijinja(v))))
        }
        _ => Value::from(()),
    }
}

fn implicit_to_minijinja(value: ImplicitValue) -> Value {
    match value {
        ImplicitValue::Bool(value) => Value::from(value),
        ImplicitValue::U64(value) => Value::from(value),
        ImplicitValue::I64(value) => Value::from(value),
        ImplicitValue::F64(value) => Value::from(value),
        _ => Value::from(()),
    }
}

fn atom_to_minijinja(atom: Atom<'_>) -> Value {
    match atom {
        Atom::Bool(value) => Value::from(value),
        Atom::Str(value) | Atom::Lexical(value) => Value::from(value.as_str()),
        Atom::Bytes(value) => Value::from_bytes(value.data().to_vec()),
        Atom::Char(value) => Value::from(value),
        Atom::U64(value) => Value::from(value),
        Atom::I64(value) => Value::from(value),
        Atom::F32(value) => Value::from(f64::from(value)),
        Atom::F64(value) => Value::from(value),
        Atom::Ext(ext) => atom_to_minijinja(ext.fallback()),
        Atom::Implicit(value) => implicit_to_minijinja(value.value()),
        _ => Value::from(()),
    }
}

/// Converts a MiniJinja value into a deser value.
///
/// Undefined values become null, plain objects are converted into their
/// string representation and iterables into sequences.
pub fn from_minijinja(value: &Value) -> Result<deser_value::Value, minijinja::Error> {
    Ok(match value.kind() {
        ValueKind::Bool => deser_value::Value::from(value.is_true()),
        ValueKind::Number => {
            let value = value.clone();
            if !value.is_integer() {
                deser_value::Value::from(f64::try_from(value)?)
            } else if let Ok(value) = i64::try_from(value.clone()) {
                deser_value::Value::from(value)
            } else if let Ok(value) = u64::try_from(value.clone()) {
                deser_value::Value::from(value)
            } else if let Ok(value) = i128::try_from(value.clone()) {
                deser_value::Value::from(value)
            } else {
                deser_value::Value::from(u128::try_from(value)?)
            }
        }
        ValueKind::String => deser_value::Value::from(value.as_str().unwrap_or_default()),
        ValueKind::Bytes => deser_value::Value::bytes(value.as_bytes().unwrap_or_default()),
        ValueKind::Seq | ValueKind::Iterable => deser_value::Value::from(
            value
                .try_iter()?
                .map(|item| from_minijinja(&item))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        ValueKind::Map => {
            let mut map = Map::new();
            for key in value.try_iter()? {
                let item = value.get_item(&key)?;
                map.insert(from_minijinja(&key)?, from_minijinja(&item)?);
            }
            deser_value::Value::from(map)
        }
        ValueKind::Plain => deser_value::Value::from(value.to_string()),
        _ => deser_value::Value::from(()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let input =
            r#"{"a":[1,-2,3.5,"x",null,true],"b":{"c":340282366920938463463374607431768211455}}"#;
        let value: deser_value::Value = deser_json::from_str(input).unwrap();
        let value = to_minijinja(&value);
        assert_eq!(value.get_attr("a").unwrap().len(), Some(6));
        let back = from_minijinja(&value).unwrap();
        assert_eq!(deser_json::to_string(&back).unwrap(), input);
    }

    #[test]
    fn test_undefined_and_objects() {
        let value = Value::from_pairs([("a", Value::UNDEFINED)]);
        let back = from_minijinja(&value).unwrap();
        assert_eq!(deser_json::to_string(&back).unwrap(), r#"{"a":null}"#);
    }
}
