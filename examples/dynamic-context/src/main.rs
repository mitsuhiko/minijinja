use std::collections::HashMap;
use std::env;

use minijinja::value::{StructObject, Value};
use minijinja::Environment;

struct DynamicContext;

impl StructObject for DynamicContext {
    fn get_field(&self, field: &str) -> Option<Value> {
        Some(match field {
            "pid" => Value::from(std::process::id()),
            "cwd" => Value::from(env::current_dir().unwrap().to_string_lossy()),
            "env" => Value::from(
                env::vars()
                    .filter(|(k, _)| k.starts_with("CARGO_") || k.starts_with("RUST_"))
                    .collect::<HashMap<String, String>>(),
            ),
            _ => return None,
        })
    }

    /// This implementation is not needed for the example.  However
    /// returning known keys here has the benefit that `{{ debug() }}`
    /// can how the context.
    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(&["pid", "cwd", "env"])
    }
}

fn main() {
    let env = Environment::new();
    println!(
        "{}",
        env.render_str(
            include_str!("template.txt"),
            Value::from_struct_object(DynamicContext)
        )
        .unwrap()
    );
}
