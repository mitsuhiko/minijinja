use std::collections::HashMap;
use std::env;

use minijinja::value::{StructObject, ValueBox};
use minijinja::Environment;

struct DynamicContext;

impl StructObject for DynamicContext {
    fn get_field(&self, key: &ValueBox) -> Option<ValueBox> {
        let field = key.as_str()?;
        Some(match field {
            "pid" => ValueBox::from(std::process::id()),
            "cwd" => ValueBox::from(env::current_dir().unwrap().to_string_lossy()),
            "env" => ValueBox::from(
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
            ValueBox::from_struct_object(DynamicContext)
        )
        .unwrap()
    );
}
