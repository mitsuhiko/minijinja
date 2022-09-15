use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

use crate::compiler::instructions::Instructions;
use crate::environment::Environment;
use crate::error::Error;
use crate::output::Output;
use crate::value::Value;
use crate::vm::Vm;

/// A handle to a compiled expression.
///
/// An expression is created via the
/// [`compile_expression`](Environment::compile_expression) method.  It provides
/// a method to evaluate the expression and return the result as value object.
/// This for instance can be used to evaluate simple expressions from user
/// provided input to implement features such as dynamic filtering.
///
/// This is usually best paired with [`context`](crate::context!) to pass
/// a single value to it.
///
/// # Example
///
/// ```rust
/// # use minijinja::{Environment, context};
/// let env = Environment::new();
/// let expr = env.compile_expression("number > 10 and number < 20").unwrap();
/// let rv = expr.eval(context!(number => 15)).unwrap();
/// assert!(rv.is_true());
/// ```
pub struct Expression<'env, 'source> {
    env: &'env Environment<'source>,
    instructions: Instructions<'source>,
}

impl<'env, 'source> fmt::Debug for Expression<'env, 'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Expression")
            .field("env", &self.env)
            .finish()
    }
}

impl<'env, 'source> Expression<'env, 'source> {
    pub fn new(
        env: &'env Environment<'source>,
        instructions: Instructions<'source>,
    ) -> Expression<'env, 'source> {
        Expression { env, instructions }
    }

    /// Evaluates the expression with some context.
    ///
    /// The result of the expression is returned as [`Value`].
    pub fn eval<S: Serialize>(&self, ctx: S) -> Result<Value, Error> {
        // reduce total amount of code faling under mono morphization into
        // this function, and share the rest in _eval.
        self._eval(Value::from_serializable(&ctx))
    }

    fn _eval(&self, root: Value) -> Result<Value, Error> {
        Ok(Vm::new(self.env)
            .eval(
                &self.instructions,
                root,
                &BTreeMap::new(),
                &mut Output::null(),
                crate::AutoEscape::None,
            )?
            .expect("expression evaluation did not leave value on stack"))
    }
}
