#![allow(clippy::let_unit_value)]
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

use minijinja::value::{from_args, Object, SeqObject, ValueBox};
use minijinja::{Environment, Error, State};

#[derive(Debug)]
struct Cycler {
    values: Vec<ValueBox>,
    idx: AtomicUsize,
}

impl fmt::Display for Cycler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "cycler")
    }
}

impl Object for Cycler {
    fn call(&self, _state: &State, args: &[ValueBox]) -> Result<ValueBox, Error> {
        // we don't want any args
        from_args(args)?;
        let idx = self.idx.fetch_add(1, Ordering::Relaxed);
        Ok(self.values[idx % self.values.len()].clone())
    }
}

fn make_cycler(_state: &State, args: Vec<ValueBox>) -> Result<ValueBox, Error> {
    Ok(ValueBox::from_object(Cycler {
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
    fn call_method(&self, _state: &State, name: &str, args: &[ValueBox]) -> Result<ValueBox, Error> {
        if name == "make_class" {
            // single string argument
            let (tag,): (&str,) = from_args(args)?;
            Ok(ValueBox::from(format!("magic-{tag}")))
        } else {
            Err(Error::new(
                minijinja::ErrorKind::UnknownMethod,
                format!("object has no method named {name}"),
            ))
        }
    }
}

struct SimpleDynamicSeq;

impl SeqObject for SimpleDynamicSeq {
    fn get_item(&self, idx: usize) -> Option<ValueBox> {
        ['a', 'b', 'c', 'd'].get(idx).copied().map(ValueBox::from)
    }

    fn item_count(&self) -> usize {
        4
    }
}

fn main() {
    let mut env = Environment::new();
    env.add_function("cycler", make_cycler);
    env.add_global("magic", ValueBox::from_object(Magic));
    env.add_global("seq", ValueBox::from_seq_object(SimpleDynamicSeq));
    env.add_template("template.html", include_str!("template.html"))
        .unwrap();

    let tmpl = env.get_template("template.html").unwrap();
    println!("{}", tmpl.render(()).unwrap());
}
