use minijinja::value::{StructObject, Value};
use minijinja::{context, Environment};
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

fn main() {
    let config = Config {
        version: env!("CARGO_PKG_VERSION"),
    };
    let mut env = Environment::new();
    env.add_template(
        "test",
        "dynamic seq: {{ seq }}\nversion: {{ config.version }}",
    )
    .unwrap();

    let tmpl = env.get_template("test").unwrap();

    let seq = &[1i32, 2, 3, 4][..];
    scope(|scope| {
        let ctx = context! {
            config => Value::from_struct_object(scope.handle(&config)),
            seq => Value::from_seq_object(scope.handle(&seq)),
        };
        println!("{}", tmpl.render(ctx).unwrap());
    });
}
