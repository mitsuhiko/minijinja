use std::convert::{TryFrom, TryInto};
use std::fmt::Write;

use crate::error::{Error, ErrorKind};
use crate::value::{Arc, ObjectKind, SeqObject, Value, ValueKind, ValueRepr};

pub enum CoerceResult<'a> {
    I128(i128, i128),
    F64(f64, f64),
    Str(&'a str, &'a str),
}

fn as_f64(value: &Value) -> Option<f64> {
    Some(match value.0 {
        ValueRepr::Bool(x) => x as i64 as f64,
        ValueRepr::U64(x) => x as f64,
        ValueRepr::U128(x) => x.0 as f64,
        ValueRepr::I64(x) => x as f64,
        ValueRepr::I128(x) => x.0 as f64,
        ValueRepr::F64(x) => x,
        _ => return None,
    })
}

pub fn coerce<'x>(a: &'x Value, b: &'x Value) -> Option<CoerceResult<'x>> {
    match (&a.0, &b.0) {
        // equal mappings are trivial
        (ValueRepr::U64(a), ValueRepr::U64(b)) => Some(CoerceResult::I128(*a as i128, *b as i128)),
        (ValueRepr::U128(a), ValueRepr::U128(b)) => {
            Some(CoerceResult::I128(a.0 as i128, b.0 as i128))
        }
        (ValueRepr::String(a, _), ValueRepr::String(b, _)) => Some(CoerceResult::Str(a, b)),
        (ValueRepr::I64(a), ValueRepr::I64(b)) => Some(CoerceResult::I128(*a as i128, *b as i128)),
        (ValueRepr::I128(a), ValueRepr::I128(b)) => Some(CoerceResult::I128(a.0, b.0)),
        (ValueRepr::F64(a), ValueRepr::F64(b)) => Some(CoerceResult::F64(*a, *b)),

        // are floats involved?
        (ValueRepr::F64(a), _) => Some(CoerceResult::F64(*a, some!(as_f64(b)))),
        (_, ValueRepr::F64(b)) => Some(CoerceResult::F64(some!(as_f64(a)), *b)),

        // everything else goes up to i128
        _ => Some(CoerceResult::I128(
            some!(i128::try_from(a.clone()).ok()),
            some!(i128::try_from(b.clone()).ok()),
        )),
    }
}

fn get_offset_and_len<F: FnOnce() -> usize>(
    start: i64,
    stop: Option<i64>,
    end: F,
) -> (usize, usize) {
    if start < 0 || stop.map_or(true, |x| x < 0) {
        let end = end();
        let start = if start < 0 {
            (end as i64 + start) as usize
        } else {
            start as usize
        };
        let stop = match stop {
            None => end,
            Some(x) if x < 0 => (end as i64 + x) as usize,
            Some(x) => x as usize,
        };
        (start, stop.saturating_sub(start))
    } else {
        (
            start as usize,
            (stop.unwrap() as usize).saturating_sub(start as usize),
        )
    }
}

pub fn slice(value: Value, start: Value, stop: Value, step: Value) -> Result<Value, Error> {
    let start: i64 = if start.is_none() {
        0
    } else {
        ok!(start.try_into())
    };
    let stop = if stop.is_none() {
        None
    } else {
        Some(ok!(i64::try_from(stop)))
    };
    let step = if step.is_none() {
        1
    } else {
        ok!(u64::try_from(step)) as usize
    };
    if step == 0 {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "cannot slice by step size of 0",
        ));
    }

    let maybe_seq = match value.0 {
        ValueRepr::String(..) => {
            let s = value.as_str().unwrap();
            let (start, len) = get_offset_and_len(start, stop, || s.chars().count());
            return Ok(Value::from(
                s.chars()
                    .skip(start)
                    .take(len)
                    .step_by(step)
                    .collect::<String>(),
            ));
        }
        ValueRepr::Undefined | ValueRepr::None => return Ok(Value::from(Vec::<Value>::new())),
        ValueRepr::Seq(ref s) => Some(&**s as &dyn SeqObject),
        ValueRepr::Dynamic(ref dy) => {
            if let ObjectKind::Seq(seq) = dy.kind() {
                Some(seq)
            } else {
                None
            }
        }
        _ => None,
    };

    match maybe_seq {
        Some(seq) => {
            let (start, len) = get_offset_and_len(start, stop, || seq.item_count());
            Ok(Value::from(
                seq.iter()
                    .skip(start)
                    .take(len)
                    .step_by(step)
                    .collect::<Vec<_>>(),
            ))
        }
        None => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("value of type {} cannot be sliced", value.kind()),
        )),
    }
}

fn int_as_value(val: i128) -> Value {
    if val as i64 as i128 == val {
        (val as i64).into()
    } else {
        val.into()
    }
}

fn impossible_op(op: &str, lhs: &Value, rhs: &Value) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "tried to use {} operator on unsupported types {} and {}",
            op,
            lhs.kind(),
            rhs.kind()
        ),
    )
}

fn failed_op(op: &str, lhs: &Value, rhs: &Value) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("unable to calculate {lhs} {op} {rhs}"),
    )
}

macro_rules! math_binop {
    ($name:ident, $int:ident, $float:tt) => {
        pub fn $name(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
            match coerce(lhs, rhs) {
                Some(CoerceResult::I128(a, b)) => match a.$int(b) {
                    Some(val) => Ok(int_as_value(val)),
                    None => Err(failed_op(stringify!($float), lhs, rhs))
                },
                Some(CoerceResult::F64(a, b)) => Ok((a $float b).into()),
                _ => Err(impossible_op(stringify!($float), lhs, rhs))
            }
        }
    }
}

pub fn add(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    match coerce(lhs, rhs) {
        Some(CoerceResult::I128(a, b)) => Ok(int_as_value(a.wrapping_add(b))),
        Some(CoerceResult::F64(a, b)) => Ok((a + b).into()),
        Some(CoerceResult::Str(a, b)) => Ok(Value::from([a, b].concat())),
        _ => Err(impossible_op("+", lhs, rhs)),
    }
}

math_binop!(sub, checked_sub, -);
math_binop!(mul, checked_mul, *);
math_binop!(rem, checked_rem_euclid, %);

pub fn div(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
        let a = some!(as_f64(lhs));
        let b = some!(as_f64(rhs));
        Some((a / b).into())
    }
    do_it(lhs, rhs).ok_or_else(|| impossible_op("/", lhs, rhs))
}

pub fn int_div(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    match coerce(lhs, rhs) {
        Some(CoerceResult::I128(a, b)) => {
            if b != 0 {
                Ok(int_as_value(a.div_euclid(b)))
            } else {
                Err(failed_op("//", lhs, rhs))
            }
        }
        Some(CoerceResult::F64(a, b)) => Ok(a.div_euclid(b).into()),
        _ => Err(impossible_op("//", lhs, rhs)),
    }
}

/// Implements a binary `pow` operation on values.
pub fn pow(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    match coerce(lhs, rhs) {
        Some(CoerceResult::I128(a, b)) => {
            match TryFrom::try_from(b).ok().and_then(|b| a.checked_pow(b)) {
                Some(val) => Ok(int_as_value(val)),
                None => Err(failed_op("**", lhs, rhs)),
            }
        }
        Some(CoerceResult::F64(a, b)) => Ok((a.powf(b)).into()),
        _ => Err(impossible_op("**", lhs, rhs)),
    }
}

/// Implements an unary `neg` operation on value.
pub fn neg(val: &Value) -> Result<Value, Error> {
    if val.kind() == ValueKind::Number {
        match val.0 {
            ValueRepr::F64(x) => Ok((-x).into()),
            _ => {
                if let Ok(x) = i128::try_from(val.clone()) {
                    Ok(int_as_value(-x))
                } else {
                    Err(Error::from(ErrorKind::InvalidOperation))
                }
            }
        }
    } else {
        Err(Error::from(ErrorKind::InvalidOperation))
    }
}

/// Attempts a string concatenation.
pub fn string_concat(mut left: Value, right: &Value) -> Value {
    match left.0 {
        // if we're a string and we have a single reference to it, we can
        // directly append into ourselves and reconstruct the value
        ValueRepr::String(ref mut s, _) => {
            write!(Arc::make_mut(s), "{right}").ok();
            left
        }
        // otherwise we use format! to concat the two values
        _ => Value::from(format!("{left}{right}")),
    }
}

/// Implements a containment operation on values.
pub fn contains(container: &Value, value: &Value) -> Result<Value, Error> {
    // Special case where if the container is undefined, it cannot hold
    // values.  For strict containment checks the vm has a special case.
    if container.is_undefined() {
        return Ok(Value::from(false));
    }
    let rv = if let Some(s) = container.as_str() {
        if let Some(s2) = value.as_str() {
            s.contains(s2)
        } else {
            s.contains(&value.to_string())
        }
    } else if let Some(seq) = container.as_seq() {
        seq.iter().any(|item| &item == value)
    } else if let ValueRepr::Map(ref map, _) = container.0 {
        let key = match value.clone().try_into_key() {
            Ok(key) => key,
            Err(_) => return Ok(Value::from(false)),
        };
        map.get(&key).is_some()
    } else {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "cannot perform a containment check on this value",
        ));
    };
    Ok(Value::from(rv))
}

#[cfg(test)]
mod tests {
    use super::*;

    use similar_asserts::assert_eq;

    #[test]
    fn test_adding() {
        let err = add(&Value::from("a"), &Value::from(42)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: tried to use + operator on unsupported types string and number"
        );

        assert_eq!(
            add(&Value::from(1), &Value::from(2)).unwrap(),
            Value::from(3)
        );
        assert_eq!(
            add(&Value::from("foo"), &Value::from("bar")).unwrap(),
            Value::from("foobar")
        );
    }

    #[test]
    fn test_subtracting() {
        let err = sub(&Value::from("a"), &Value::from(42)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: tried to use - operator on unsupported types string and number"
        );

        let err = sub(&Value::from("foo"), &Value::from("bar")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: tried to use - operator on unsupported types string and string"
        );

        assert_eq!(
            sub(&Value::from(2), &Value::from(1)).unwrap(),
            Value::from(1)
        );
    }

    #[test]
    fn test_dividing() {
        let err = div(&Value::from("a"), &Value::from(42)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: tried to use / operator on unsupported types string and number"
        );

        let err = div(&Value::from("foo"), &Value::from("bar")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: tried to use / operator on unsupported types string and string"
        );

        assert_eq!(
            div(&Value::from(100), &Value::from(2)).unwrap(),
            Value::from(50.0)
        );
    }

    #[test]
    fn test_concat() {
        assert_eq!(
            string_concat(Value::from("foo"), &Value::from(42)),
            Value::from("foo42")
        );
        assert_eq!(
            string_concat(Value::from(23), &Value::from(42)),
            Value::from("2342")
        );
    }
}
