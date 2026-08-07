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
    values: Vec<Value>,
}

impl Tuple {
    /// Creates a tuple from values.
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    /// Consumes the tuple and returns its values.
    pub fn into_vec(self) -> Vec<Value> {
        self.values
    }
}

impl From<Vec<Value>> for Tuple {
    fn from(values: Vec<Value>) -> Self {
        Self::new(values)
    }
}

impl<const N: usize> From<[Value; N]> for Tuple {
    fn from(values: [Value; N]) -> Self {
        Self::new(values.into())
    }
}

impl From<&[Value]> for Tuple {
    fn from(values: &[Value]) -> Self {
        Self::new(values.to_vec())
    }
}

impl Deref for Tuple {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl Object for Tuple {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.values.get(key.as_usize()?).cloned()
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Seq(self.values.len())
    }

    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        for (idx, value) in self.values.iter().enumerate() {
            if idx != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{value:?}")?;
        }
        if self.values.len() == 1 {
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
