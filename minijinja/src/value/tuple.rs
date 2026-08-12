use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use super::{Enumerator, Object, ObjectRepr, Value};

/// A sequence that preserves Jinja's tuple semantics and rendering.
///
/// Tuples behave as sequences in the template engine, but render with
/// parentheses and preserve the trailing comma of a one-item tuple.
#[derive(Clone, Debug, Default)]
pub struct Tuple {
    values: TupleValues,
}

#[derive(Clone, Debug, Default)]
enum TupleValues {
    #[default]
    Empty,
    One([Value; 1]),
    Two([Value; 2]),
    Many(Vec<Value>),
}

impl Tuple {
    /// Creates a tuple from values.
    pub fn new(mut values: Vec<Value>) -> Self {
        let values = match values.len() {
            0 => TupleValues::Empty,
            1 => TupleValues::One([values.pop().unwrap()]),
            2 => {
                let second = values.pop().unwrap();
                let first = values.pop().unwrap();
                TupleValues::Two([first, second])
            }
            _ => TupleValues::Many(values),
        };
        Self { values }
    }

    /// Consumes the tuple and returns its values.
    pub fn into_vec(self) -> Vec<Value> {
        match self.values {
            TupleValues::Empty => vec![],
            TupleValues::One(values) => values.into(),
            TupleValues::Two(values) => values.into(),
            TupleValues::Many(values) => values,
        }
    }
}

impl From<Vec<Value>> for Tuple {
    fn from(values: Vec<Value>) -> Self {
        Self::new(values)
    }
}

impl<const N: usize> From<[Value; N]> for Tuple {
    fn from(values: [Value; N]) -> Self {
        let mut values = values.into_iter();
        let values = match N {
            0 => TupleValues::Empty,
            1 => TupleValues::One([values.next().unwrap()]),
            2 => TupleValues::Two([values.next().unwrap(), values.next().unwrap()]),
            _ => TupleValues::Many(values.collect()),
        };
        Self { values }
    }
}

impl From<&[Value]> for Tuple {
    fn from(values: &[Value]) -> Self {
        let values = match values {
            [] => TupleValues::Empty,
            [value] => TupleValues::One([value.clone()]),
            [first, second] => TupleValues::Two([first.clone(), second.clone()]),
            values => TupleValues::Many(values.to_vec()),
        };
        Self { values }
    }
}

impl Deref for Tuple {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        match &self.values {
            TupleValues::Empty => &[],
            TupleValues::One(values) => values,
            TupleValues::Two(values) => values,
            TupleValues::Many(values) => values,
        }
    }
}

impl Object for Tuple {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.get(key.as_usize()?).cloned()
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        match &self.values {
            TupleValues::Empty => Enumerator::Empty,
            TupleValues::One(values) => Enumerator::Iter(Box::new(values.clone().into_iter())),
            TupleValues::Two(values) => Enumerator::Iter(Box::new(values.clone().into_iter())),
            TupleValues::Many(values) => Enumerator::Seq(values.len()),
        }
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(self.len())
    }

    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        for (idx, value) in self.iter().enumerate() {
            if idx != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{value:?}")?;
        }
        if self.len() == 1 {
            f.write_str(",")?;
        }
        f.write_str(")")
    }
}

impl From<Tuple> for Value {
    fn from(value: Tuple) -> Self {
        Value::from_object(value)
    }
}

impl From<&Tuple> for Value {
    fn from(value: &Tuple) -> Self {
        Value::from(value.clone())
    }
}

macro_rules! impl_value_tuple {
    ($($name:ident),+) => {
        impl<$($name: Into<Value>),+> From<($($name,)+)> for Value {
            #[allow(non_snake_case)]
            fn from(value: ($($name,)+)) -> Self {
                let ($($name,)+) = value;
                Value::from(Tuple::from([$($name.into(),)+]))
            }
        }

        impl<$($name),+> From<&($($name,)+)> for Value
        where
            $($name: Clone + Into<Value>),+
        {
            fn from(value: &($($name,)+)) -> Self {
                Value::from(value.clone())
            }
        }
    };
}

impl_value_tuple!(A);
impl_value_tuple!(A, B);
impl_value_tuple!(A, B, C);
impl_value_tuple!(A, B, C, D);
impl_value_tuple!(A, B, C, D, E);
impl_value_tuple!(A, B, C, D, E, F);
impl_value_tuple!(A, B, C, D, E, F, G);
impl_value_tuple!(A, B, C, D, E, F, G, H);
impl_value_tuple!(A, B, C, D, E, F, G, H, I);
impl_value_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_value_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_value_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
