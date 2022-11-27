use std::{env, fmt};

use minijinja::value::{Object, StructObject, Value};
use minijinja::{context, Environment, Error, ErrorKind, State};
use minijinja_stack_ref::scope;

struct Config {
    manifest_dir: &'static str,
    version: &'static str,
}

impl StructObject for Config {
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(&["manifest_dir", "version"][..])
    }

    fn get_field(&self, field: &str) -> Option<Value> {
        match field {
            "manifest_dir" => Some(Value::from(self.manifest_dir)),
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
    let env = Environment::new();

    // values on the stack we want to pass dynamically to the template
    // without serialization
    let config = Config {
        manifest_dir: env!("CARGO_MANIFEST_DIR"),
        version: env!("CARGO_PKG_VERSION"),
    };
    let items = [1i32, 2, 3, 4];

    scope(|scope| {
        let ctx = context! {
            config => scope.struct_object_ref(&config),
            utils => scope.object_ref(&Utils),
            items => scope.seq_object_ref(&items[..]),
        };
        println!(
            "{}",
            env.render_str(include_str!("template.txt"), ctx).unwrap()
        );
    });
}
