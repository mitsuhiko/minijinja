#![allow(clippy::let_unit_value)]
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use minijinja::value::{from_args, Enumeration, Object, ObjectRepr, Value};
use minijinja::{Environment, Error, State};

#[derive(Debug)]
struct Cycler {
    values: Vec<Value>,
    idx: AtomicUsize,
}

impl Object for Cycler {
    fn call(self: &Arc<Self>, _state: &State, args: &[Value]) -> Result<Value, Error> {
        // we don't want any args
        from_args(args)?;
        let idx = self.idx.fetch_add(1, Ordering::Relaxed);
        Ok(self.values[idx % self.values.len()].clone())
    }
}

fn make_cycler(_state: &State, args: Vec<Value>) -> Result<Value, Error> {
    Ok(Value::from_object(Cycler {
        values: args,
        idx: AtomicUsize::new(0),
    }))
}

#[derive(Debug)]
struct Magic;

impl fmt::Display for Magic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "magic")
    }
}

impl Object for Magic {
    fn call_method(
        self: &Arc<Self>,
        _state: &State,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        if name == "make_class" {
            // single string argument
            let (tag,): (&str,) = from_args(args)?;
            Ok(Value::from(format!("magic-{tag}")))
        } else {
            Err(Error::new(
                minijinja::ErrorKind::UnknownMethod,
                format!("object has no method named {name}"),
            ))
        }
    }
}

#[derive(Debug)]
struct SimpleDynamicSeq;

impl Object for SimpleDynamicSeq {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, idx: &Value) -> Option<Value> {
        let idx = idx.as_usize()?;
        ['a', 'b', 'c', 'd'].get(idx).copied().map(Value::from)
    }

    fn enumeration(self: &Arc<Self>) -> Enumeration {
        Enumeration::Sized(4)
    }
}

fn main() {
    let mut env = Environment::new();
    env.add_function("cycler", make_cycler);
    env.add_global("magic", Value::from_object(Magic));
    env.add_global("seq", Value::from_object(SimpleDynamicSeq));
    // TODO: add this back
    //env.add_global("real_iter", Value::from_iterator((0..10).chain(20..30)));
    env.add_template("template.html", include_str!("template.html"))
        .unwrap();

    let tmpl = env.get_template("template.html").unwrap();
    println!("{}", tmpl.render(()).unwrap());
}
