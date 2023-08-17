use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::error::{Error, ErrorKind};
use crate::output::Output;
use crate::utils::AutoEscape;
use crate::value::{
    KeyRef, MapType, Object, ObjectKind, StringType, StructObject, Value, ValueRepr,
};
use crate::vm::state::State;
use crate::vm::Vm;

pub(crate) struct MacroData {
    pub name: Arc<str>,
    pub arg_spec: Vec<Arc<str>>,
    // because values need to be 'static, we can't hold a reference to the
    // instructions that declared the macro.  Instead of that we place the
    // reference to the macro instruction (and the jump offset) in the
    // state under `state.macros`.
    pub macro_ref_id: usize,
    pub state_id: isize,
    pub closure: Value,
    pub caller_reference: bool,
}

pub(crate) struct Macro {
    // the extra level of Arc here is necessary for recursive calls only.
    // For more information have a look at the call() method.
    pub data: Arc<MacroData>,
}

impl fmt::Debug for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<macro {}>", self.data.name)
    }
}

impl Object for Macro {
    fn kind(&self) -> ObjectKind<'_> {
        ObjectKind::Struct(self)
    }

    fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
        // we can only call macros that point to loaded template state.
        if state.id != self.data.state_id {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "cannot call this macro. template state went away.",
            ));
        }

        let (args, kwargs) = match args.last() {
            Some(Value(ValueRepr::Map(kwargs, MapType::Kwargs))) => {
                (&args[..args.len() - 1], Some(kwargs))
            }
            _ => (args, None),
        };

        if args.len() > self.data.arg_spec.len() {
            return Err(Error::from(ErrorKind::TooManyArguments));
        }

        let mut kwargs_used = BTreeSet::new();
        let mut arg_values = Vec::with_capacity(self.data.arg_spec.len());
        for (idx, name) in self.data.arg_spec.iter().enumerate() {
            let kwarg = match kwargs {
                Some(kwargs) => kwargs.get(&KeyRef::Str(name)),
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

        let caller = if self.data.caller_reference {
            kwargs_used.insert("caller");
            Some(
                kwargs
                    .and_then(|x| x.get(&KeyRef::Str("caller")).cloned())
                    .unwrap_or(Value::UNDEFINED),
            )
        } else {
            None
        };

        if let Some(kwargs) = kwargs {
            for key in kwargs.keys().filter_map(|x| x.as_str()) {
                if !kwargs_used.contains(key) {
                    return Err(Error::new(
                        ErrorKind::TooManyArguments,
                        format!("unknown keyword argument `{key}`"),
                    ));
                }
            }
        }

        let (instructions, offset) = &state.macros[self.data.macro_ref_id];
        let vm = Vm::new(state.env());
        let mut rv = String::new();
        let mut out = Output::with_string(&mut rv);

        // If a macro is self referential we need to put a reference to ourselves
        // there.  Unfortunately because we only have a &self reference here, we
        // cannot bump our own refcount.  Instead we need to wrap the macro data
        // into an extra level of Arc to avoid unnecessary clones.
        let closure = self.data.closure.clone();

        // This requires some explanation here.  Because we get the state as &State and
        // not &mut State we are required to create a new state here.  This is unfortunate
        // but makes the calling interface more convenient for the rest of the system.
        // Because macros cannot return anything other than strings (most importantly they)
        // can't return other macros this is however not an issue, as modifications in the
        // macro cannot leak out.
        ok!(vm.eval_macro(
            instructions,
            *offset,
            closure,
            caller,
            &mut out,
            state,
            arg_values
        ));

        Ok(if !matches!(state.auto_escape(), AutoEscape::None) {
            Value::from_safe_string(rv)
        } else {
            Value::from(rv)
        })
    }
}

impl StructObject for Macro {
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(&["name", "arguments", "caller"][..])
    }

    fn get_field(&self, name: &str) -> Option<Value> {
        match name {
            "name" => Some(Value(ValueRepr::String(
                self.data.name.clone(),
                StringType::Normal,
            ))),
            "arguments" => Some(Value::from(
                self.data
                    .arg_spec
                    .iter()
                    .map(|x| Value(ValueRepr::String(x.clone(), StringType::Normal)))
                    .collect::<Vec<_>>(),
            )),
            "caller" => Some(Value::from(self.data.caller_reference)),
            _ => None,
        }
    }
}
