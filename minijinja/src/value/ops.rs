use crate::error::{Error, ErrorKind};
use crate::utils::{collect_untrusted, untrusted_size_hint};
use crate::value::merge_object::MergeSeq;
use crate::value::{
    checked, DynObject, MaybeInvalid, ObjectRepr, Tuple, Value, ValueKind, ValueRepr,
};
use crate::vm::State;

const MIN_I128_AS_POS_U128: u128 = 170141183460469231731687303715884105728;
const MAX_REPEATED_STRING_LEN: usize = 100_000_000;
const MAX_REPEATED_SEQUENCE_LEN: usize = 100_000_000;

/// Iterator wrapper that provides exact size hints for iterators with known length.
pub(crate) struct LenIterWrap<I: Send + Sync>(pub(crate) usize, pub(crate) I);

impl<I: Iterator<Item = Value> + Send + Sync> Iterator for LenIterWrap<I> {
    type Item = Value;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.1.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0, Some(self.0))
    }
}

pub enum CoerceResult<'a> {
    I128(i128, i128),
    F64(f64, f64),
    Str(&'a str, &'a str),
}

pub(crate) fn as_f64(value: &Value, lossy: bool) -> Option<f64> {
    macro_rules! checked {
        ($expr:expr, $ty:ty) => {{
            let rv = $expr as f64;
            return if lossy || rv as $ty == $expr {
                Some(rv)
            } else {
                None
            };
        }};
    }

    Some(match value.0 {
        ValueRepr::Bool(x) => x as i64 as f64,
        ValueRepr::U64(x) => checked!(x, u64),
        ValueRepr::U128(x) => checked!(x.0, u128),
        ValueRepr::I64(x) => checked!(x, i64),
        ValueRepr::I128(x) => checked!(x.0, i128),
        ValueRepr::F64(x) => x,
        _ => return None,
    })
}

pub fn coerce<'x>(a: &'x Value, b: &'x Value, lossy: bool) -> Option<CoerceResult<'x>> {
    match (&a.0, &b.0) {
        // equal mappings are trivial
        (ValueRepr::U64(a), ValueRepr::U64(b)) => Some(CoerceResult::I128(*a as i128, *b as i128)),
        (ValueRepr::U128(a), ValueRepr::U128(b)) => {
            Some(CoerceResult::I128(a.0 as i128, b.0 as i128))
        }
        (ValueRepr::String(a, _), ValueRepr::String(b, _)) => Some(CoerceResult::Str(a, b)),
        (ValueRepr::SmallStr(a), ValueRepr::SmallStr(b)) => {
            Some(CoerceResult::Str(a.as_str(), b.as_str()))
        }
        (ValueRepr::SmallStr(a), ValueRepr::String(b, _)) => Some(CoerceResult::Str(a.as_str(), b)),
        (ValueRepr::String(a, _), ValueRepr::SmallStr(b)) => Some(CoerceResult::Str(a, b.as_str())),
        (ValueRepr::I64(a), ValueRepr::I64(b)) => Some(CoerceResult::I128(*a as i128, *b as i128)),
        (ValueRepr::I128(a), ValueRepr::I128(b)) => Some(CoerceResult::I128(a.0, b.0)),
        (ValueRepr::F64(a), ValueRepr::F64(b)) => Some(CoerceResult::F64(*a, *b)),

        // are floats involved?
        (ValueRepr::F64(a), _) => Some(CoerceResult::F64(*a, some!(as_f64(b, lossy)))),
        (_, ValueRepr::F64(b)) => Some(CoerceResult::F64(some!(as_f64(a, lossy)), *b)),

        // everything else goes up to i128
        _ => Some(CoerceResult::I128(
            some!(i128::try_from(a.clone()).ok()),
            some!(i128::try_from(b.clone()).ok()),
        )),
    }
}

fn get_offset_and_len<F: FnOnce() -> usize>(
    start: Option<i64>,
    stop: Option<i64>,
    end: F,
) -> (usize, usize) {
    let start = start.unwrap_or(0);
    if start < 0 || stop.map_or(true, |x| x < 0) {
        let end = end();
        let start = if start < 0 {
            std::cmp::max(0, end as i64 + start) as usize
        } else {
            start as usize
        };
        let stop = match stop {
            None => end,
            Some(x) if x < 0 => std::cmp::max(0, end as i64 + x) as usize,
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

fn range_step_backwards(
    start: Option<i64>,
    stop: Option<i64>,
    step: usize,
    end: usize,
) -> impl Iterator<Item = usize> {
    // Like Python, clamp both bounds to `-1..=end - 1` and walk down from
    // `start` to `stop`, excluding `stop`.
    let end = end as i64;
    let clamp = |idx: i64| {
        if idx < 0 {
            (end + idx).max(-1)
        } else {
            idx.min(end - 1)
        }
    };
    let start = start.map_or(end - 1, clamp);
    let stop = stop.map_or(-1, clamp);
    let length = if start > stop {
        ((start - stop - 1) as u64 / step as u64 + 1) as usize
    } else {
        0
    };
    (0..length).map(move |i| (start as u64 - i as u64 * step as u64) as usize)
}

/// Slices a value.
///
/// Slicing an iterable backwards needs to consume it.  This happens right
/// away so that the items are charged against the fuel of the state.
pub fn slice(
    state: &State,
    value: Value,
    start: Value,
    stop: Value,
    step: Value,
) -> Result<Value, Error> {
    let start = if start.is_none() {
        None
    } else {
        Some(ok!(start.try_into()))
    };
    let stop = if stop.is_none() {
        None
    } else {
        Some(ok!(i64::try_from(stop)))
    };
    let step = if step.is_none() {
        1i64
    } else {
        ok!(i64::try_from(step))
    };
    if step == 0 {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "cannot slice by step size of 0",
        ));
    }

    let kind = value.kind();
    let is_tuple = value.is_tuple();
    let error = Err(Error::new(
        ErrorKind::InvalidOperation,
        format!("value of type {kind} cannot be sliced"),
    ));

    match value.0 {
        ValueRepr::String(..) | ValueRepr::SmallStr(_) => {
            let s = value.as_str().unwrap();
            if step == 1 {
                // the common case of a contiguous slice can copy the bytes
                // directly once the char offsets are known.
                let ascii = s.is_ascii();
                let (start, len) = get_offset_and_len(start, stop, || {
                    if ascii {
                        s.len()
                    } else {
                        s.chars().count()
                    }
                });
                let (byte_start, byte_end) = if ascii {
                    let byte_start = start.min(s.len());
                    (byte_start, byte_start.saturating_add(len).min(s.len()))
                } else {
                    let mut offsets = s.char_indices().map(|x| x.0).chain(Some(s.len()));
                    let byte_start = offsets.nth(start).unwrap_or(s.len());
                    let byte_end = match len {
                        0 => byte_start,
                        len => offsets.nth(len - 1).unwrap_or(s.len()),
                    };
                    (byte_start, byte_end)
                };
                Ok(Value::from(&s[byte_start..byte_end]))
            } else if step > 0 {
                let (start, len) = get_offset_and_len(start, stop, || s.chars().count());
                Ok(Value::from(
                    s.chars()
                        .skip(start)
                        .take(len)
                        .step_by(step as usize)
                        .collect::<String>(),
                ))
            } else {
                let chars: Vec<char> = s.chars().collect();
                Ok(Value::from(
                    range_step_backwards(start, stop, -step as usize, chars.len())
                        .map(move |i| chars[i])
                        .collect::<String>(),
                ))
            }
        }
        ValueRepr::Bytes(ref b) => {
            if step > 0 {
                let (start, len) = get_offset_and_len(start, stop, || b.len());
                Ok(Value::from_bytes(
                    b.iter()
                        .skip(start)
                        .take(len)
                        .step_by(step as usize)
                        .copied()
                        .collect(),
                ))
            } else {
                Ok(Value::from_bytes(
                    range_step_backwards(start, stop, -step as usize, b.len())
                        .map(|i| b[i])
                        .collect::<Vec<u8>>(),
                ))
            }
        }
        ValueRepr::Undefined(..) | ValueRepr::None => Ok(Value::from(Vec::<Value>::new())),
        ValueRepr::Object(obj) if matches!(obj.repr(), ObjectRepr::Seq | ObjectRepr::Iterable) => {
            if is_tuple {
                let values = match obj.try_iter() {
                    Some(iter) => ok!(checked(state.fuel_iter(iter)).try_collect_vec()),
                    None => Vec::new(),
                };
                let values: Vec<Value> = if step > 0 {
                    let (start, len) = get_offset_and_len(start, stop, || values.len());
                    values
                        .into_iter()
                        .skip(start)
                        .take(len)
                        .step_by(step as usize)
                        .collect()
                } else {
                    range_step_backwards(start, stop, -step as usize, values.len())
                        .map(|idx| values[idx].clone())
                        .collect()
                };
                return Ok(Value::from(Tuple::from(values)));
            }

            if step > 0 {
                let len = obj.enumerator_len().unwrap_or_default();
                let (start, len) = get_offset_and_len(start, stop, || len);
                Ok(Value::make_object_iterable(obj, move |obj| {
                    if let Some(iter) = obj.try_iter() {
                        Box::new(iter.skip(start).take(len).step_by(step as usize))
                    } else {
                        Box::new(None.into_iter())
                    }
                }))
            } else if let (ObjectRepr::Seq, Some(len)) = (obj.repr(), obj.enumerator_len()) {
                // sequences can be indexed, so they can be sliced lazily
                Ok(Value::make_object_iterable(obj, move |obj| {
                    Box::new(
                        range_step_backwards(start, stop, -step as usize, len)
                            .map(move |idx| obj.get_value(&Value::from(idx)).unwrap_or_default()),
                    )
                }))
            } else {
                let items = match obj.try_iter() {
                    Some(iter) => collect_untrusted(state.fuel_iter(iter)),
                    None => Vec::new(),
                };
                if let Some(err) = items.last().and_then(|x| x.check().err()) {
                    return Err(err);
                }
                let indexes = range_step_backwards(start, stop, -step as usize, items.len());
                let items = collect_untrusted(indexes.map(|idx| items[idx].clone()));
                Ok(Value::make_object_iterable(items, |items| {
                    Box::new(items.iter().cloned())
                }))
            }
        }
        _ => error,
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
    .with_undefined_origin(lhs)
    .with_undefined_origin(rhs)
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
            match coerce(lhs, rhs, true) {
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

fn seq_concat_len(lhs: &Value, rhs: &Value) -> Option<usize> {
    lhs.len()?.checked_add(rhs.len()?)
}

fn materialize_seq_concat(lhs: &Value, rhs: &Value, len: usize) -> Result<Value, Error> {
    let mut rv = Vec::with_capacity(untrusted_size_hint(len));
    for item in ok!(lhs.try_iter())
        .checked()
        .chain(ok!(rhs.try_iter()).checked())
    {
        rv.push(ok!(item));
    }
    Ok(Value::from(rv))
}

pub fn add(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    if lhs.is_tuple() || rhs.is_tuple() {
        if lhs.is_tuple() && rhs.is_tuple() {
            let mut values = Vec::with_capacity(untrusted_size_hint(
                seq_concat_len(lhs, rhs).unwrap_or_default(),
            ));
            for item in ok!(lhs.try_iter())
                .checked()
                .chain(ok!(rhs.try_iter()).checked())
            {
                values.push(ok!(item));
            }
            return Ok(Value::from(Tuple::from(values)));
        }
        return Err(impossible_op("+", lhs, rhs));
    }

    if matches!(lhs.kind(), ValueKind::Seq | ValueKind::Iterable)
        && matches!(rhs.kind(), ValueKind::Seq | ValueKind::Iterable)
    {
        let values = vec![lhs.clone(), rhs.clone()];
        let depth = MergeSeq::depth_for_values(&values);

        // Keep sequence concatenation lazy by default.  The one case where we
        // materialize eagerly is when repeated `seq = seq + [x]` has built a
        // chain deep enough that later iteration or drop would risk one native
        // stack frame per concatenation.  Only do that for sized operands;
        // unsized iterables may represent streams and must not be consumed, so
        // `MergeSeq` flattens only its own lazy structure instead.
        if depth > MergeSeq::MAX_DEPTH {
            if let Some(len) = seq_concat_len(lhs, rhs) {
                return materialize_seq_concat(lhs, rhs, len);
            }
        }

        return Ok(Value::from_object(MergeSeq::new_iterable(values)));
    }
    match coerce(lhs, rhs, true) {
        Some(CoerceResult::I128(a, b)) => a
            .checked_add(b)
            .ok_or_else(|| failed_op("+", lhs, rhs))
            .map(int_as_value),
        Some(CoerceResult::F64(a, b)) => Ok((a + b).into()),
        Some(CoerceResult::Str(a, b)) => Ok(Value::from([a, b].concat())),
        _ => Err(impossible_op("+", lhs, rhs)),
    }
}

math_binop!(sub, checked_sub, -);

fn int_div_rem_floor(a: i128, b: i128) -> Option<(i128, i128)> {
    let mut quotient = a.checked_div(b)?;
    let mut remainder = a.checked_rem(b)?;
    if remainder != 0 && (remainder < 0) != (b < 0) {
        quotient = quotient.checked_sub(1)?;
        remainder = remainder.checked_add(b)?;
    }
    Some((quotient, remainder))
}

fn float_div_rem_floor(a: f64, b: f64) -> (f64, f64) {
    let mut remainder = a % b;
    let mut quotient = (a - remainder) / b;
    if remainder != 0.0 {
        if (remainder < 0.0) != (b < 0.0) {
            remainder += b;
            quotient -= 1.0;
        }
    } else {
        remainder = 0.0f64.copysign(b);
    }
    if quotient != 0.0 {
        let mut floored = quotient.floor();
        if quotient - floored > 0.5 {
            floored += 1.0;
        }
        quotient = floored;
    } else {
        quotient = 0.0f64.copysign(a / b);
    }
    (quotient, remainder)
}

pub fn rem(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    match coerce(lhs, rhs, true) {
        Some(CoerceResult::I128(a, b)) if b != 0 => {
            if a == i128::MIN && b == -1 {
                Ok(Value::from(0))
            } else {
                int_div_rem_floor(a, b)
                    .map(|(_, remainder)| int_as_value(remainder))
                    .ok_or_else(|| failed_op("%", lhs, rhs))
            }
        }
        Some(CoerceResult::F64(a, b)) if b != 0.0 => Ok(Value::from(float_div_rem_floor(a, b).1)),
        Some(CoerceResult::I128(..) | CoerceResult::F64(..)) => Err(failed_op("%", lhs, rhs)),
        _ => Err(impossible_op("%", lhs, rhs)),
    }
}

pub fn mul(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    if let Some((s, safe, n)) = lhs
        .as_str()
        .map(|s| (s, lhs.is_safe(), rhs))
        .or_else(|| rhs.as_str().map(|s| (s, rhs.is_safe(), lhs)))
    {
        let n = ok!(repeat_count(n).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                "strings can only be multiplied with integers",
            )
        }));
        if !matches!(s.len().checked_mul(n), Some(len) if len <= MAX_REPEATED_STRING_LEN) {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "repeated string is too large",
            ));
        }
        let repeated = s.repeat(n);
        return Ok(if safe {
            Value::from_safe_string(repeated)
        } else {
            Value::from(repeated)
        });
    } else if let Some((seq, n)) = lhs
        .as_object()
        .map(|s| (s, rhs))
        .or_else(|| rhs.as_object().map(|s| (s, lhs)))
        .filter(|x| matches!(x.0.repr(), ObjectRepr::Iterable | ObjectRepr::Seq))
    {
        return repeat_iterable(n, seq);
    }

    match coerce(lhs, rhs, true) {
        Some(CoerceResult::I128(a, b)) => match a.checked_mul(b) {
            Some(val) => Ok(int_as_value(val)),
            None => Err(failed_op(stringify!(*), lhs, rhs)),
        },
        Some(CoerceResult::F64(a, b)) => Ok((a * b).into()),
        _ => Err(impossible_op(stringify!(*), lhs, rhs)),
    }
}

/// Like in Python, a negative count repeats zero times.
fn repeat_count(n: &Value) -> Option<usize> {
    let n = i128::try_from(n.clone()).ok()?;
    Some(usize::try_from(n.max(0)).unwrap_or(usize::MAX))
}

fn repeat_iterable(n: &Value, seq: &DynObject) -> Result<Value, Error> {
    let n = ok!(repeat_count(n).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            "sequences and iterables can only be multiplied with integers",
        )
    }));

    let len = ok!(seq.enumerator_len().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            "cannot repeat unsized iterables",
        )
    }));

    if len == 0 || n == 0 {
        return Ok(if seq.downcast_ref::<Tuple>().is_some() {
            Value::from(Tuple::default())
        } else {
            Value::from(Vec::<Value>::new())
        });
    }

    let repeated_len = ok!(len
        .checked_mul(n)
        .filter(|&len| len <= MAX_REPEATED_SEQUENCE_LEN)
        .ok_or_else(|| Error::new(
            ErrorKind::InvalidOperation,
            "repeated sequence is too large"
        )));

    if let Some(tuple) = seq.downcast_ref::<Tuple>() {
        let mut values = Vec::with_capacity(repeated_len);
        for _ in 0..n {
            values.extend(tuple.iter().cloned());
        }
        return Ok(Value::from(Tuple::from(values)));
    }

    // This is not optimal.  We only query the enumerator for the length once
    // but we support repeated iteration.  We could both lie about our length
    // here and we could actually deal with an object that changes how much
    // data it returns.  This is not really permissible so we won't try to
    // improve on this here.
    Ok(Value::make_object_iterable(seq.clone(), move |seq| {
        Box::new(LenIterWrap(
            repeated_len,
            (0..n).flat_map(move |_| {
                seq.try_iter().unwrap_or_else(|| {
                    Box::new(
                        std::iter::repeat(Value::from(Error::new(
                            ErrorKind::InvalidOperation,
                            "iterable did not iterate against expectations",
                        )))
                        .take(len),
                    )
                })
            }),
        ))
    }))
}

pub fn div(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    let Some(a) = as_f64(lhs, true) else {
        return Err(impossible_op("/", lhs, rhs));
    };
    let Some(b) = as_f64(rhs, true) else {
        return Err(impossible_op("/", lhs, rhs));
    };
    if b == 0.0 {
        Err(failed_op("/", lhs, rhs))
    } else {
        Ok((a / b).into())
    }
}

pub fn int_div(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    match coerce(lhs, rhs, true) {
        Some(CoerceResult::I128(a, b)) if b != 0 => {
            if a == i128::MIN && b == -1 {
                Ok(Value::from(MIN_I128_AS_POS_U128))
            } else {
                int_div_rem_floor(a, b)
                    .map(|(quotient, _)| int_as_value(quotient))
                    .ok_or_else(|| failed_op("//", lhs, rhs))
            }
        }
        Some(CoerceResult::F64(a, b)) if b != 0.0 => Ok(Value::from(float_div_rem_floor(a, b).0)),
        Some(CoerceResult::I128(..) | CoerceResult::F64(..)) => Err(failed_op("//", lhs, rhs)),
        _ => Err(impossible_op("//", lhs, rhs)),
    }
}

/// Implements a binary `pow` operation on values.
pub fn pow(lhs: &Value, rhs: &Value) -> Result<Value, Error> {
    match coerce(lhs, rhs, true) {
        // A negative exponent gives a fractional result, like in Python.
        // Zero to a negative power fails like a division by zero.
        Some(CoerceResult::I128(a, b)) if b < 0 => {
            if a == 0 {
                Err(failed_op("**", lhs, rhs))
            } else {
                Ok((a as f64).powf(b as f64).into())
            }
        }
        Some(CoerceResult::I128(a, b)) => {
            match TryFrom::try_from(b).ok().and_then(|b| a.checked_pow(b)) {
                Some(val) => Ok(int_as_value(val)),
                None => Err(failed_op("**", lhs, rhs)),
            }
        }
        Some(CoerceResult::F64(a, b)) if a == 0.0 && b < 0.0 => Err(failed_op("**", lhs, rhs)),
        Some(CoerceResult::F64(a, b)) => Ok((a.powf(b)).into()),
        _ => Err(impossible_op("**", lhs, rhs)),
    }
}

/// Implements an unary `neg` operation on value.
pub fn neg(val: &Value) -> Result<Value, Error> {
    if val.kind() == ValueKind::Number {
        match val.0 {
            ValueRepr::F64(x) => Ok((-x).into()),
            // special case for the largest i128 that can still be
            // represented.
            ValueRepr::U128(x) if x.0 == MIN_I128_AS_POS_U128 => {
                Ok(Value::from(MIN_I128_AS_POS_U128))
            }
            _ => {
                if let Ok(x) = i128::try_from(val.clone()) {
                    x.checked_mul(-1)
                        .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "overflow"))
                        .map(int_as_value)
                } else {
                    Err(Error::from(ErrorKind::InvalidOperation))
                }
            }
        }
    } else {
        Err(Error::from(ErrorKind::InvalidOperation).with_undefined_origin(val))
    }
}

/// Attempts a string concatenation.
pub fn string_concat(left: Value, right: &Value) -> Value {
    // Fast path for the common case of two strings: skip the formatter.
    if let (Some(l), Some(r)) = (plain_str(&left), plain_str(right)) {
        let mut rv = String::with_capacity(l.len() + r.len());
        rv.push_str(l);
        rv.push_str(r);
        return Value::from(rv);
    }
    Value::from(format!("{left}{right}"))
}

fn plain_str(v: &Value) -> Option<&str> {
    match v.0 {
        ValueRepr::String(ref s, _) => Some(s),
        ValueRepr::SmallStr(ref s) => Some(s.as_str()),
        _ => None,
    }
}

/// Estimates how many bytes the result of a binary operation will need.
///
/// This only covers operations that create new strings or materialized
/// sequences and is used to refuse an allocation before it happens.  The
/// result is in fuel units (bytes, and list slots of
/// [`SLOT_COST`] bytes).
pub(crate) fn binop_alloc_estimate(op: &str, lhs: &Value, rhs: &Value) -> u64 {
    fn str_len(v: &Value) -> Option<u64> {
        match v.0 {
            ValueRepr::String(ref s, _) => Some(s.len() as u64),
            ValueRepr::SmallStr(ref s) => Some(s.as_str().len() as u64),
            ValueRepr::Bytes(ref b) => Some(b.len() as u64),
            _ => None,
        }
    }

    match op {
        "mul" => {
            let (repeated, n) = if lhs.as_usize().is_some() {
                (rhs, lhs)
            } else {
                (lhs, rhs)
            };
            let n = n.as_usize().unwrap_or(0) as u64;
            if let Some(len) = str_len(repeated) {
                len.saturating_mul(n)
            } else if repeated.is_tuple() {
                (repeated.len().unwrap_or(0) as u64)
                    .saturating_mul(n)
                    .saturating_mul(SLOT_COST)
            } else {
                // repeating lists is lazy
                0
            }
        }
        "add" | "string_concat" => match (str_len(lhs), str_len(rhs)) {
            (Some(a), Some(b)) => a.saturating_add(b),
            (Some(a), None) if op == "string_concat" => a,
            (None, Some(b)) if op == "string_concat" => b,
            _ if lhs.is_tuple() && rhs.is_tuple() => (lhs.len().unwrap_or(0) as u64)
                .saturating_add(rhs.len().unwrap_or(0) as u64)
                .saturating_mul(SLOT_COST),
            _ => 0,
        },
        _ => 0,
    }
}

/// The cost of a slot in a list for the purpose of fuel and size estimates.
pub(crate) const SLOT_COST: u64 = 32;

/// Checks that there is enough fuel left to perform a binary operation.
#[inline(always)]
pub(crate) fn ensure_fuel_for_binop(
    _state: &State,
    _op: &str,
    _lhs: &Value,
    _rhs: &Value,
) -> Result<(), Error> {
    #[cfg(feature = "fuel")]
    {
        if _state.fuel_tracker().is_some() {
            return _state.ensure_fuel(binop_alloc_estimate(_op, _lhs, _rhs));
        }
    }
    Ok(())
}

/// Charges fuel for comparing two values.
///
/// Comparing containers needs to visit their items, so this charges for the
/// items of the shorter one.
#[inline(always)]
pub(crate) fn charge_fuel_for_compare(
    _state: &State,
    _lhs: &Value,
    _rhs: &Value,
) -> Result<(), Error> {
    #[cfg(feature = "fuel")]
    {
        if _state.fuel_tracker().is_some() {
            if let (Some(a), Some(b)) = (_lhs.as_object(), _rhs.as_object()) {
                if !a.is_same_object(b) {
                    if let (Some(a_len), Some(b_len)) = (a.enumerator_len(), b.enumerator_len()) {
                        return _state.charge_items(a_len.min(b_len));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Charges fuel for looking up an item by index in an iterable.
///
/// Iterables cannot be indexed, so the lookup needs to iterate up to the
/// index.
#[inline(always)]
pub(crate) fn charge_fuel_for_get_item(
    _state: &State,
    _container: &Value,
    _key: &Value,
) -> Result<(), Error> {
    #[cfg(feature = "fuel")]
    {
        if _state.fuel_tracker().is_some() {
            if let Some(obj) = _container.as_object() {
                if obj.repr() == ObjectRepr::Iterable {
                    if let Some(idx) = _key.as_i64() {
                        let mut n = usize::try_from(idx.unsigned_abs()).unwrap_or(usize::MAX);
                        if let Some(len) = obj.enumerator_len() {
                            n = n.min(len);
                        }
                        return _state.charge_items(n);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Implements a containment operation on values.
///
/// If a state is passed, the items visited are charged against the fuel.
pub fn contains(state: Option<&State>, container: &Value, value: &Value) -> Result<Value, Error> {
    // Special case where if the container is undefined, it cannot hold
    // values.  For strict containment checks the vm has a special case.
    if container.is_undefined() {
        return Ok(Value::from(false));
    }
    let rv = if let Some(s) = container.as_str() {
        if let Some(state) = state {
            ok!(state.charge_fuel(s.len() as u64));
        }
        if let Some(s2) = value.as_str() {
            s.contains(s2)
        } else {
            s.contains(&value.to_string())
        }
    } else if let ValueRepr::Object(ref obj) = container.0 {
        match obj.repr() {
            ObjectRepr::Plain => false,
            ObjectRepr::Map => obj.get_value(value).is_some(),
            ObjectRepr::Seq | ObjectRepr::Iterable => {
                let mut found = false;
                if let Some(iter) = obj.try_iter() {
                    for item in iter {
                        if let Some(state) = state {
                            ok!(state.charge_items(1));
                        }
                        ok!(item.check());
                        if item == *value {
                            found = true;
                            break;
                        }
                    }
                }
                found
            }
        }
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
    fn test_neg() {
        let err = neg(&Value::from(i128::MIN)).unwrap_err();
        assert_eq!(err.to_string(), "invalid operation: overflow");
    }

    #[test]
    fn test_string_repeat_size_limit() {
        let err = mul(&Value::from("ab"), &Value::from(50_000_001usize)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: repeated string is too large"
        );

        let err = mul(&Value::from("ab"), &Value::from(usize::MAX)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: repeated string is too large"
        );
    }

    #[test]
    fn test_pow_negative_int_exponent() {
        assert_eq!(
            pow(&Value::from(2), &Value::from(-1)).unwrap(),
            Value::from(0.5)
        );
        assert_eq!(
            pow(&Value::from(-2), &Value::from(-2)).unwrap(),
            Value::from(0.25)
        );
        assert_eq!(
            pow(&Value::from(2), &Value::from(3)).unwrap(),
            Value::from(8)
        );

        let err = pow(&Value::from(0), &Value::from(-1)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: unable to calculate 0 ** -1"
        );
        let err = pow(&Value::from(0.0), &Value::from(-1)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: unable to calculate 0.0 ** -1"
        );
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

        let err = add(&Value::from(i128::MAX), &Value::from(1)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid operation: unable to calculate 170141183460469231731687303715884105727 + 1"
        );
    }

    #[test]
    #[cfg_attr(
        target_os = "wasi",
        ignore = "std::thread::Builder::spawn is unsupported on WASI"
    )]
    fn test_repeated_seq_add_does_not_overflow_stack() {
        // Regression test for repeated sequence concatenation, for example a
        // chat-template accumulator `messages = messages + [m]` applied for
        // many turns.  `+` may stay lazy, but must not build an unbounded chain
        // of lazy iterables, otherwise enumerating the result (for example via
        // `{{ messages | length }}` -> `Value::len()`) recurses one native frame
        // per concatenation and overflows the stack.  Run on a small (1 MiB)
        // stack so the regression aborts deterministically rather than
        // depending on the platform default stack size.
        let handle = std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let n = 5000;
                let mut acc = Value::from(Vec::<Value>::new());
                for i in 0..n {
                    acc = add(&acc, &Value::from(vec![Value::from(i)])).unwrap();
                }
                // The crash path: `|length` -> `Value::len()`.
                assert_eq!(acc.len(), Some(n));
                // Iteration must also work and stay correct.
                assert_eq!(acc.try_iter().unwrap().count(), n);

                let mut acc = Value::make_iterable(|| (0i64..).take_while(|_| false));
                for i in 0..n {
                    acc = add(&acc, &Value::from(vec![Value::from(i)])).unwrap();
                }
                // Unsized iterables cannot be eagerly materialized to cut off
                // nesting, so iterating the lazy concat object must flatten its
                // own nested concat nodes without recursion.
                assert_eq!(acc.len(), None);
                assert_eq!(acc.try_iter().unwrap().count(), n);
            })
            .unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_sized_iterable_add_stays_lazy() {
        struct CountingIter {
            next_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
            idx: usize,
            len: usize,
        }

        impl Iterator for CountingIter {
            type Item = Value;

            fn next(&mut self) -> Option<Self::Item> {
                if self.idx == self.len {
                    return None;
                }
                let idx = self.idx;
                self.idx += 1;
                self.next_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Some(Value::from(idx as i64))
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                let remaining = self.len - self.idx;
                (remaining, Some(remaining))
            }
        }

        let next_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let lhs_next_count = next_count.clone();
        let lhs = Value::make_iterable(move || CountingIter {
            next_count: lhs_next_count.clone(),
            idx: 0,
            len: 2,
        });
        let rhs = Value::from(vec![Value::from(2), Value::from(3)]);

        let res = add(&lhs, &rhs).unwrap();
        assert_eq!(next_count.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Length remains known without consuming either side.
        assert_eq!(res.len(), Some(4));
        assert_eq!(next_count.load(std::sync::atomic::Ordering::SeqCst), 0);

        let got: Vec<i64> = res
            .try_iter()
            .unwrap()
            .map(|v| i64::try_from(v).unwrap())
            .collect();
        assert_eq!(got, vec![0, 1, 2, 3]);
        assert_eq!(next_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn test_unsized_iterable_add_stays_lazy() {
        // An unsized iterable (no exact size hint) must NOT be eagerly
        // materialized; the lazy-chaining fallback must still apply so adding
        // to a potentially-unbounded stream stays lazy.
        let unsized_iter = Value::make_iterable(|| (0i64..).take_while(|&x| x < 3));
        assert_eq!(unsized_iter.len(), None, "precondition: operand is unsized");
        let res = add(&unsized_iter, &Value::from(vec![Value::from(99)])).unwrap();
        // Result is still a lazy iterable with unknown length...
        assert_eq!(res.kind(), ValueKind::Iterable);
        assert_eq!(res.len(), None);
        // ...but iterates to the correct concatenated contents.
        let got: Vec<i64> = res
            .try_iter()
            .unwrap()
            .map(|v| i64::try_from(v).unwrap())
            .collect();
        assert_eq!(got, vec![0, 1, 2, 99]);
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

        assert_eq!(
            int_div(&Value::from(i128::MIN), &Value::from(-1i128)).unwrap(),
            Value::from(MIN_I128_AS_POS_U128)
        );
        assert_eq!(
            rem(&Value::from(i128::MIN), &Value::from(-1i128)).unwrap(),
            Value::from(0)
        );
    }

    #[test]
    fn test_floor_division_and_modulo() {
        for (a, b, quotient, remainder) in [
            (7, 3, 2, 1),
            (-7, 3, -3, 2),
            (7, -3, -3, -2),
            (-7, -3, 2, -1),
            (-9, 3, -3, 0),
        ] {
            assert_eq!(
                int_div(&Value::from(a), &Value::from(b)).unwrap(),
                Value::from(quotient)
            );
            assert_eq!(
                rem(&Value::from(a), &Value::from(b)).unwrap(),
                Value::from(remainder)
            );
        }

        for (a, b, quotient, remainder) in [
            (-7.0, 3.0, -3.0, 2.0),
            (7.0, -3.0, -3.0, -2.0),
            (-7.0, -3.0, 2.0, -1.0),
            (-7.5, 2.0, -4.0, 0.5),
            (1.0, 0.1, 9.0, 0.09999999999999995),
        ] {
            assert_eq!(
                int_div(&Value::from(a), &Value::from(b)).unwrap(),
                Value::from(quotient)
            );
            assert_eq!(
                rem(&Value::from(a), &Value::from(b)).unwrap(),
                Value::from(remainder)
            );
        }

        assert!(int_div(&Value::from(1.0), &Value::from(0.0)).is_err());
        assert!(rem(&Value::from(1.0), &Value::from(0.0)).is_err());
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
        assert_eq!(
            string_concat(Value::from("foo"), &Value::from("bar")),
            Value::from("foobar")
        );
        assert_eq!(
            string_concat(
                Value::from("x".repeat(40)),
                &Value::from_safe_string("<y>".into())
            ),
            Value::from(format!("{}<y>", "x".repeat(40)))
        );
    }

    #[test]
    fn test_slicing() {
        let env = crate::Environment::new();
        let state = env.empty_state();
        let slice = |value, start, stop, step| super::slice(&state, value, start, stop, step);
        let v = Value::from(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

        // [::] - full slice
        assert_eq!(
            slice(v.clone(), Value::from(()), Value::from(()), Value::from(())).unwrap(),
            Value::from(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
        );

        // [::2] - every 2nd element
        assert_eq!(
            slice(v.clone(), Value::from(()), Value::from(()), Value::from(2)).unwrap(),
            Value::from(vec![0, 2, 4, 6, 8])
        );

        // [1:2:2] - slice with start, stop, step
        assert_eq!(
            slice(v.clone(), Value::from(1), Value::from(2), Value::from(2)).unwrap(),
            Value::from(vec![1])
        );

        // [::-2] - reverse with step of 2
        assert_eq!(
            slice(v.clone(), Value::from(()), Value::from(()), Value::from(-2)).unwrap(),
            Value::from(vec![9, 7, 5, 3, 1])
        );

        // [:-8:] - from index 0 to -8
        assert_eq!(
            slice(v.clone(), Value::from(()), Value::from(-8), Value::from(())).unwrap(),
            Value::from(vec![0, 1])
        );

        // [-8::] - from index -8 to the end
        assert_eq!(
            slice(v.clone(), Value::from(-8), Value::from(()), Value::from(())).unwrap(),
            Value::from(vec![2, 3, 4, 5, 6, 7, 8, 9])
        );

        // [-11::] - from index -11 to the end, which is the same as [::]
        // because the start index is before the start of the vector
        assert_eq!(
            slice(
                v.clone(),
                Value::from(-11),
                Value::from(()),
                Value::from(())
            )
            .unwrap(),
            Value::from(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
        );

        // [:-11:] - from index -11 to the end, which is the same as [:0:]
        // because the end index is before the start of the vector
        assert_eq!(
            slice(
                v.clone(),
                Value::from(()),
                Value::from(-11),
                Value::from(())
            )
            .unwrap(),
            Value::from(Vec::<usize>::new())
        );

        // [2::-2] - from index 2 to start, reverse with step of 2
        assert_eq!(
            slice(v.clone(), Value::from(2), Value::from(()), Value::from(-2)).unwrap(),
            Value::from(vec![2, 0])
        );

        // [4:2:-2] - from index 4 to 2, reverse with step of 2
        assert_eq!(
            slice(v.clone(), Value::from(4), Value::from(2), Value::from(-2)).unwrap(),
            Value::from(vec![4])
        );

        // [8:3:-2] - from index 8 to 3, reverse with step of 2
        assert_eq!(
            slice(v.clone(), Value::from(8), Value::from(3), Value::from(-2)).unwrap(),
            Value::from(vec![8, 6, 4])
        );
    }

    #[test]
    fn test_string_slicing() {
        let env = crate::Environment::new();
        let state = env.empty_state();
        let slice = |value, start, stop, step| super::slice(&state, value, start, stop, step);
        let s = Value::from("abcdefghij");

        // [::] - full slice
        assert_eq!(
            slice(s.clone(), Value::from(()), Value::from(()), Value::from(())).unwrap(),
            Value::from("abcdefghij")
        );

        // [::2] - every 2nd character
        assert_eq!(
            slice(s.clone(), Value::from(()), Value::from(()), Value::from(2)).unwrap(),
            Value::from("acegi")
        );

        // [1:2:2] - slice with start, stop, step
        assert_eq!(
            slice(s.clone(), Value::from(1), Value::from(2), Value::from(2)).unwrap(),
            Value::from("b")
        );

        // [::-2] - reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(()), Value::from(()), Value::from(-2)).unwrap(),
            Value::from("jhfdb")
        );

        // [2::-2] - from index 2 to start, reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(2), Value::from(()), Value::from(-2)).unwrap(),
            Value::from("ca")
        );

        // [4:2:-2] - from index 4 to 2, reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(4), Value::from(2), Value::from(-2)).unwrap(),
            Value::from("e")
        );

        // [8:3:-2] - from index 8 to 3, reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(8), Value::from(3), Value::from(-2)).unwrap(),
            Value::from("ige")
        );
    }

    #[test]
    fn test_bytes_slicing() {
        let env = crate::Environment::new();
        let state = env.empty_state();
        let slice = |value, start, stop, step| super::slice(&state, value, start, stop, step);
        let s = Value::from_bytes(b"abcdefghij".to_vec());

        // [::] - full slice
        assert_eq!(
            slice(s.clone(), Value::from(()), Value::from(()), Value::from(())).unwrap(),
            Value::from_bytes(b"abcdefghij".to_vec())
        );

        // [::2] - every 2nd character
        assert_eq!(
            slice(s.clone(), Value::from(()), Value::from(()), Value::from(2)).unwrap(),
            Value::from_bytes(b"acegi".to_vec())
        );

        // [1:2:2] - slice with start, stop, step
        assert_eq!(
            slice(s.clone(), Value::from(1), Value::from(2), Value::from(2)).unwrap(),
            Value::from_bytes(b"b".to_vec())
        );

        // [::-2] - reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(()), Value::from(()), Value::from(-2)).unwrap(),
            Value::from_bytes(b"jhfdb".to_vec())
        );

        // [2::-2] - from index 2 to start, reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(2), Value::from(()), Value::from(-2)).unwrap(),
            Value::from_bytes(b"ca".to_vec())
        );

        // [4:2:-2] - from index 4 to 2, reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(4), Value::from(2), Value::from(-2)).unwrap(),
            Value::from_bytes(b"e".to_vec())
        );

        // [8:3:-2] - from index 8 to 3, reverse with step of 2
        assert_eq!(
            slice(s.clone(), Value::from(8), Value::from(3), Value::from(-2)).unwrap(),
            Value::from_bytes(b"ige".to_vec())
        );
    }
}
