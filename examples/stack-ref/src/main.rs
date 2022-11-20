use std::{env, fmt};

use minijinja::value::{Object, StructObject, Value};
use minijinja::{context, Environment, Error, ErrorKind, State};
use minijinja_stack_ref::scope;

struct Config {
    version: &'static str,
}

impl StructObject for Config {
    fn get_field(&self, field: &str) -> Option<Value> {
        match field {
            "version" => Some(Value::from(self.version)),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Utils;

impl fmt::Display for Utils {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<utils>")
    }
}

impl Object for Utils {
    fn call_method(&self, _state: &State, name: &str, _args: &[Value]) -> Result<Value, Error> {
        match name {
            "get_cwd" => Ok(Value::from(env::current_dir().unwrap().to_string_lossy())),
            _ => Err(Error::from(ErrorKind::UnknownMethod)),
        }
    }
}

fn main() {
    let config = Config {
        version: env!("CARGO_PKG_VERSION"),
    };
    let utils = Utils;
    let env = Environment::new();
    scope(|scope| {
        let ctx = context! {
            config => Value::from_struct_object(scope.handle(&config)),
            utils => Value::from_object(scope.handle(&utils)),
        };
        println!(
            "{}",
            env.render_str(
                "version: {{ config.version }}\ncwd: {{ utils.get_cwd() }}",
                ctx
            )
            .unwrap()
        );
    });
}
