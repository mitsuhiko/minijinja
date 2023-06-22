use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use minijinja::value::{StructObject, Value, ValueKind};
use minijinja::{context, Environment};

struct TrackedContext {
    enclosed: Value,
    resolved: Arc<Mutex<HashSet<String>>>,
}

impl StructObject for TrackedContext {
    fn get_field(&self, name: &str) -> Option<Value> {
        let mut resolved = self.resolved.lock().unwrap();
        if !resolved.contains(name) {
            resolved.insert(name.to_string());
        }
        self.enclosed
            .get_attr(name)
            .ok()
            .filter(|x| !x.is_undefined())
    }

    fn fields(&self) -> Vec<Arc<str>> {
        if self.enclosed.kind() == ValueKind::Map {
            if let Ok(keys) = self.enclosed.try_iter() {
                return keys.filter_map(|x| Arc::<str>::try_from(x).ok()).collect();
            }
        }
        Vec::new()
    }
}

pub fn track_context(ctx: Value) -> (Value, Arc<Mutex<HashSet<String>>>) {
    let resolved = Arc::new(Mutex::default());
    (
        Value::from_struct_object(TrackedContext {
            enclosed: ctx,
            resolved: resolved.clone(),
        }),
        resolved,
    )
}

fn main() {
    let mut env = Environment::new();
    env.add_global("global", true);
    let template = env
        .template_from_str(
            "name={{ name }}; undefined_value={{ undefined_value }}; global={{ global }}",
        )
        .unwrap();

    let (ctx, resolved) = track_context(context! {
        name => "John",
        unused => 42
    });

    println!("rendered: {}", template.render(ctx).unwrap());
    println!("resolved: {:?}", resolved.lock().unwrap());
}
