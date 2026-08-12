use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::error::{Error, ErrorKind};
use crate::output::Output;
use crate::utils::AutoEscape;
use crate::value::{Enumerator, Kwargs, Object, Value};
use crate::vm;
use crate::vm::state::State;

pub(crate) struct Macro {
    pub name: Value,
    pub arg_spec: Vec<Value>,
    // Values need to be 'static, so the macro cannot directly borrow the
    // instructions that declared it.  The instruction stream is registered
    // once on the render state and identified by its stable address.
    pub instructions_id: usize,
    pub offset: u32,
    pub state_id: isize,
    pub closure: Option<usize>,
    pub caller_reference: bool,
}

impl fmt::Debug for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<macro {}>", self.name)
    }
}

impl Macro {
    fn prepare_args(&self, args: &[Value]) -> Result<(Vec<Value>, Option<Value>), Error> {
        let (args, kwargs) = match args.last() {
            Some(last) => match Kwargs::extract(last) {
                Some(kwargs) => (&args[..args.len() - 1], Some(kwargs)),
                None => (args, None),
            },
            _ => (args, None),
        };

        if args.len() > self.arg_spec.len() {
            return Err(Error::from(ErrorKind::TooManyArguments));
        }

        let mut kwargs_used = BTreeSet::new();
        let mut arg_values = Vec::with_capacity(self.arg_spec.len());
        for (idx, name) in self.arg_spec.iter().enumerate() {
            let name = match name.as_str() {
                Some(name) => name,
                None => {
                    arg_values.push(Value::UNDEFINED);
                    continue;
                }
            };
            let kwarg: Option<&Value> = match kwargs {
                Some(ref kwargs) => kwargs.get(name).ok(),
                _ => None,
            };
            arg_values.push(match (args.get(idx), kwarg) {
                (Some(_), Some(_)) => {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("duplicate argument `{name}`"),
                    ))
                }
                (Some(arg), None) => arg.clone(),
                (None, Some(kwarg)) => {
                    kwargs_used.insert(name as &str);
                    kwarg.clone()
                }
                (None, None) => Value::UNDEFINED,
            });
        }

        let caller = if self.caller_reference {
            kwargs_used.insert("caller");
            Some(
                kwargs
                    .as_ref()
                    .and_then(|x| x.get("caller").ok())
                    .unwrap_or(Value::UNDEFINED),
            )
        } else {
            None
        };

        if let Some(kwargs) = kwargs {
            for key in kwargs.values.keys().filter_map(|x| x.as_str()) {
                if !kwargs_used.contains(key) {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("unknown keyword argument `{key}`"),
                    ));
                }
            }
        }

        Ok((arg_values, caller))
    }
}

impl Object for Macro {
    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Str(&["name", "arguments", "caller"])
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        Some(match some!(key.as_str()) {
            "name" => self.name.clone(),
            "arguments" => Value::from_iter(self.arg_spec.iter().cloned()),
            "caller" => Value::from(self.caller_reference),
            _ => return None,
        })
    }

    fn call(self: &Arc<Self>, state: &mut State<'_, '_>, args: &[Value]) -> Result<Value, Error> {
        if state.id != self.state_id {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "cannot call this macro. template state went away.",
            ));
        }

        let (arg_values, caller) = ok!(self.prepare_args(args));
        let mut rv = String::new();
        ok!(vm::eval_macro(
            state,
            self.instructions_id,
            self.offset,
            &mut Output::new(&mut rv),
            self.closure,
            caller,
            arg_values
        ));

        Ok(if !matches!(state.auto_escape(), AutoEscape::None) {
            Value::from_safe_string(rv)
        } else {
            Value::from(rv)
        })
    }

    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<macro {}>", self.name)
    }
}
