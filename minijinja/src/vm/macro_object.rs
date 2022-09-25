use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::error::{Error, ErrorKind};
use crate::key::Key;
use crate::output::Output;
use crate::utils::AutoEscape;
use crate::value::{MapType, Object, StringType, Value, ValueRepr};
use crate::vm::state::State;
use crate::vm::Vm;

pub(crate) struct Macro {
    pub name: Arc<String>,
    pub arg_spec: Vec<Arc<String>>,
    // because values need to be 'static, we can't hold a reference to the
    // instructions that declared the macro.  Instead of that we place the
    // reference to the macro instruction (and the jump offset) in the
    // state under `state.macros`.
    pub macro_ref_id: usize,
    pub closure: Value,
}

impl fmt::Debug for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<macro {}>", self.name)
    }
}

impl Object for Macro {
    fn attributes(&self) -> &[&str] {
        &["name", "arguments"][..]
    }

    fn get_attr(&self, name: &str) -> Option<Value> {
        match name {
            "name" => Some(Value(ValueRepr::String(
                self.name.clone(),
                StringType::Normal,
            ))),
            "arguments" => Some(Value::from(
                self.arg_spec
                    .iter()
                    .map(|x| Value(ValueRepr::String(x.clone(), StringType::Normal)))
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        }
    }

    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        let (args, kwargs) = match args.last() {
            Some(Value(ValueRepr::Map(kwargs, MapType::Kwargs))) => {
                (&args[..args.len() - 1], Some(kwargs))
            }
            _ => (args, None),
        };

        if args.len() > self.arg_spec.len() {
            return Err(Error::from(ErrorKind::TooManyArguments));
        }

        let mut kwargs_used = BTreeSet::new();
        let mut arg_values = Vec::with_capacity(self.arg_spec.len());
        for (idx, name) in self.arg_spec.iter().enumerate() {
            let kwarg = match kwargs {
                Some(kwargs) => kwargs.get(&Key::Str(name)),
                _ => None,
            };
            arg_values.push(match (args.get(idx), kwarg) {
                (Some(_), Some(_)) => {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("duplicate argument `{}`", name),
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

        if let Some(kwargs) = kwargs {
            for key in kwargs.keys().filter_map(|x| x.as_str()) {
                if !kwargs_used.contains(key) {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("unknown keyword argument `{}`", key),
                    ));
                }
            }
        }

        let (instructions, offset) = &state.macros[self.macro_ref_id];
        let vm = Vm::new(state.env());
        let mut rv = String::new();
        let mut out = Output::with_string(&mut rv);

        // This requires some explanation here.  Because we get the state as &State and
        // not &mut State we are required to create a new state here.  This is unfortunate
        // but makes the calling interface more convenient for the rest of the system.
        // Because macros cannot return anything other than strings (most importantly they)
        // can't return other macros this is however not an issue, as modifications in the
        // macro cannot leak out.
        ok!(vm.eval_macro(
            instructions,
            *offset,
            self.closure.clone(),
            &mut out,
            state,
            arg_values,
        ));

        Ok(if !matches!(state.auto_escape(), AutoEscape::None) {
            Value::from_safe_string(rv)
        } else {
            Value::from(rv)
        })
    }
}
