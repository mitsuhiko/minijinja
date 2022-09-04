#![allow(clippy::let_unit_value)]
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

use minijinja::value::{FunctionArgs, Object, Value};
use minijinja::{Environment, Error, State};

#[derive(Debug)]
struct Cycler {
    values: Vec<Value>,
    idx: AtomicUsize,
}

impl fmt::Display for Cycler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "cycler")
    }
}

impl Object for Cycler {
    fn call(&self, _state: &State, args: &[Value]) -> Result<Value, Error> {
        let _: () = FunctionArgs::from_values(args)?;
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
    fn call_method(&self, _state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        if name == "make_class" {
            let (tag,): (String,) = FunctionArgs::from_values(args)?;
            Ok(Value::from(format!("magic-{}", tag)))
        } else {
            Err(Error::new(
                minijinja::ErrorKind::ImpossibleOperation,
                format!("object has no method named {}", name),
            ))
        }
    }
}

fn main() {
    let mut env = Environment::new();
    env.add_function("cycler", make_cycler);
    env.add_global("magic", Value::from_object(Magic));
    env.add_template("template.html", include_str!("template.html"))
        .unwrap();

    let tmpl = env.get_template("template.html").unwrap();
    println!("{}", tmpl.render(&()).unwrap());
}
