use std::convert::TryFrom;
use std::fmt::Write;

use crate::error::{Error, ErrorKind};
use crate::value::{Arc, Value, ValueKind, ValueRepr};

pub enum CoerceResult {
    I128(i128, i128),
    F64(f64, f64),
    String(String, String),
}

fn as_f64(value: &Value) -> Option<f64> {
    Some(match value.0 {
        ValueRepr::Bool(x) => x as i64 as f64,
        ValueRepr::U64(x) => x as f64,
        ValueRepr::U128(ref x) => **x as f64,
        ValueRepr::I64(x) => x as f64,
        ValueRepr::I128(ref x) => **x as f64,
        ValueRepr::F64(x) => x,
        _ => return None,
    })
}

pub fn coerce(a: &Value, b: &Value) -> Option<CoerceResult> {
    match (&a.0, &b.0) {
        // equal mappings are trivial
        (ValueRepr::U64(a), ValueRepr::U64(b)) => Some(CoerceResult::I128(*a as i128, *b as i128)),
        (ValueRepr::U128(a), ValueRepr::U128(b)) => {
            Some(CoerceResult::I128(**a as i128, **b as i128))
        }
        (ValueRepr::String(a), ValueRepr::String(b)) => {
            Some(CoerceResult::String(a.to_string(), b.to_string()))
        }
        (ValueRepr::I64(a), ValueRepr::I64(b)) => Some(CoerceResult::I128(*a as i128, *b as i128)),
        (ValueRepr::I128(ref a), ValueRepr::I128(ref b)) => Some(CoerceResult::I128(**a, **b)),
        (ValueRepr::F64(a), ValueRepr::F64(b)) => Some(CoerceResult::F64(*a, *b)),

        // are floats involved?
        (ValueRepr::F64(a), _) => Some(CoerceResult::F64(*a, as_f64(b)?)),
        (_, ValueRepr::F64(b)) => Some(CoerceResult::F64(as_f64(a)?, *b)),

        // everything else goes up to i128
        _ => Some(CoerceResult::I128(
            i128::try_from(a.clone()).ok()?,
            i128::try_from(b.clone()).ok()?,
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

macro_rules! math_binop {
    ($name:ident, $int:ident, $float:tt) => {
        pub fn $name(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
            fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
                match coerce(lhs, rhs)? {
                    CoerceResult::I128(a, b) => Some(int_as_value(a.$int(b))),
                    CoerceResult::F64(a, b) => Some((a $float b).into()),
                    _ => None
                }
            }
            do_it(lhs, rhs).ok_or_else(|| {
                impossible_op(stringify!($float), lhs, rhs)
            })
        }
    }
}

pub fn add(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
        match coerce(lhs, rhs)? {
            CoerceResult::I128(a, b) => Some(int_as_value(a.wrapping_add(b))),
            CoerceResult::F64(a, b) => Some((a + b).into()),
            CoerceResult::String(a, b) => Some(Value::from([a, b].concat())),
        }
    }
    do_it(lhs, rhs).ok_or_else(|| impossible_op("+", lhs, rhs))
}

math_binop!(sub, wrapping_sub, -);
math_binop!(mul, wrapping_mul, *);
math_binop!(rem, wrapping_rem_euclid, %);

pub fn div(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
        let a = as_f64(lhs)?;
        let b = as_f64(rhs)?;
        Some((a / b).into())
    }
    do_it(lhs, rhs).ok_or_else(|| impossible_op("/", lhs, rhs))
}

pub fn int_div(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
        match coerce(lhs, rhs)? {
            CoerceResult::I128(a, b) => Some(int_as_value(a.div_euclid(b))),
            CoerceResult::F64(a, b) => Some(a.div_euclid(b).into()),
            CoerceResult::String(_, _) => None,
        }
    }
    do_it(lhs, rhs).ok_or_else(|| impossible_op("//", lhs, rhs))
}

/// Implements a binary `pow` operation on values.
pub fn pow(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    pub fn do_it(lhs: &Value, rhs: &Value) -> Option<Value> {
        match coerce(lhs, rhs)? {
            CoerceResult::I128(a, b) => Some(int_as_value(a.pow(TryFrom::try_from(b).ok()?))),
            CoerceResult::F64(a, b) => Some((a.powf(b)).into()),
            CoerceResult::String(_, _) => None,
        }
    }
    do_it(lhs, rhs).ok_or_else(|| impossible_op("**", lhs, rhs))
}

/// Implements an unary `neg` operation on value.
pub fn neg(val: &Value) -> Result<Value, Error> {
    fn do_it(val: &Value) -> Option<Value> {
        match val.0 {
            ValueRepr::F64(x) => return Some((-x).into()),
            _ => {
                if let Ok(x) = i128::try_from(val.clone()) {
                    return Some(int_as_value(-x));
                }
            }
        }
        None
    }

    if val.kind() != ValueKind::Number {
        Err(Error::from(ErrorKind::InvalidOperation))
    } else {
        do_it(val).ok_or_else(|| Error::from(ErrorKind::InvalidOperation))
    }
}

/// Attempts a string concatenation.
pub fn string_concat(mut left: Value, right: &Value) -> Value {
    match left.0 {
        // if we're a string and we have a single reference to it, we can
        // directly append into ourselves and reconstruct the value
        ValueRepr::String(ref mut s) => {
            write!(Arc::make_mut(s), "{}", right).ok();
            left
        }
        // otherwise we use format! to concat the two values
        _ => Value::from(format!("{}{}", left, right)),
    }
}

/// Implements a containment operation on values.
pub fn contains(container: &Value, value: &Value) -> Result<Value, Error> {
    match container.0 {
        ValueRepr::Seq(ref values) => Ok(Value::from(values.contains(value))),
        ValueRepr::Map(ref map) => {
            let key = match value.clone().try_into_key() {
                Ok(key) => key,
                Err(_) => return Ok(Value::from(false)),
            };
            return Ok(Value::from(map.get(&key).is_some()));
        }
        ValueRepr::String(ref s) | ValueRepr::SafeString(ref s) => {
            return Ok(Value::from(if let Some(s2) = value.as_str() {
                s.contains(&s2)
            } else {
                s.contains(&value.to_string())
            }));
        }
        _ => Err(Error::new(
            ErrorKind::InvalidOperation,
            "cannot perform a containment check on this value",
        )),
    }
}

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
