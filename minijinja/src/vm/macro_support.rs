use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::compiler::instructions::Instructions;
use crate::error::{Error, ErrorKind};
use crate::key::Key;
use crate::output::Output;
use crate::utils::AutoEscape;
use crate::value::{Object, Value, ValueRepr};
use crate::vm::state::State;
use crate::vm::Vm;

#[derive(Clone)]
pub(crate) struct MacroRef<'vm, 'env> {
    pub instructions: &'vm Instructions<'env>,
    pub offset: usize,
}

pub(crate) struct Macro {
    pub name: Arc<String>,
    pub arg_spec: Vec<Arc<String>>,
    // because values need to be 'static, we can't hold a refernece to the
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
            "name" => Some(Value(ValueRepr::String(self.name.clone()))),
            "arguments" => Some(Value::from(
                self.arg_spec
                    .iter()
                    .map(|x| Value(ValueRepr::String(x.clone())))
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        }
    }

    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        let (args, kwargs) = match args.last() {
            Some(Value(ValueRepr::Kwargs(kwargs))) => (&args[..args.len() - 1], Some(kwargs)),
            _ => (args, None),
        };

        if args.len() > self.arg_spec.len() {
            return Err(Error::from(ErrorKind::TooManyArguments));
        }

        let mut kwargs_used = BTreeSet::new();
        let mut arg_values = Vec::with_capacity(self.arg_spec.len());
        for (idx, name) in self.arg_spec.iter().enumerate() {
            let kwarg = match kwargs {
                Some(kwargs) => kwargs.get(&Key::Str(name)).map(|x| (name.as_str(), x)),
                _ => None,
            };
            arg_values.push(match (args.get(idx), kwarg) {
                (Some(_), Some(_)) => {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("duplicate argument `{}`", name),
                    ));
                }
                (Some(arg), None) => arg.clone(),
                (None, Some((name, kwarg))) => {
                    kwargs_used.insert(name);
                    kwarg.clone()
                }
                (None, None) => Value::UNDEFINED,
            });
        }

        if let Some(kwargs) = kwargs {
            let extra_args = kwargs
                .keys()
                .filter_map(|x| x.as_str().filter(|x| !kwargs_used.contains(x)))
                .collect::<Vec<_>>();
            match &extra_args[..] {
                &[] => {}
                &[first] => {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("unknown keyword argument {}", first),
                    ))
                }
                rest => {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("unknown keyword arguments {}", rest.join(", ")),
                    ))
                }
            }
        }

        let macro_ref = &state.macros[self.macro_ref_id];
        let vm = Vm::new(state.env());
        let mut rv = String::new();
        let mut out = Output::with_string(&mut rv);

        // This requires some explanation here.  Because we get the state as &State and
        // not &mut State we are required to create a new state here.  This is unfortunate
        // but makes the calling interface more convenient for the rest of the system.
        // Because macros cannot return anything other than strings (most importantly they)
        // can't return other macros this is however not an issue, as modifications in the
        // macro cannot leak out.
        vm.eval_macro(
            macro_ref.instructions,
            macro_ref.offset,
            self.closure.clone(),
            &mut out,
            state,
            arg_values,
        )?;
        Ok(if !matches!(state.auto_escape(), AutoEscape::None) {
            Value::from_safe_string(rv)
        } else {
            Value::from(rv)
        })
    }
}
