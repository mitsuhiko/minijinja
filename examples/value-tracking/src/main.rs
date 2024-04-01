use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use minijinja::value::{Enumerator, Object, Value, ValueKind};
use minijinja::{context, Environment};

#[derive(Debug)]
struct TrackedContext {
    enclosed: Value,
    resolved: Arc<Mutex<HashSet<String>>>,
}

impl Object for TrackedContext {
    fn get_value(self: &Arc<Self>, name: &Value) -> Option<Value> {
        let mut resolved = self.resolved.lock().unwrap();
        if let Some(name) = name.as_str() {
            if !resolved.contains(name) {
                resolved.insert(name.to_string());
            }
            self.enclosed
                .get_attr(name)
                .ok()
                .filter(|x| !x.is_undefined())
        } else {
            None
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        if self.enclosed.kind() == ValueKind::Map {
            if let Ok(keys) = self.enclosed.try_iter() {
                return Enumerator::Values(keys.collect());
            }
        }
        Enumerator::Seq(0)
    }
}

pub fn track_context(ctx: Value) -> (Value, Arc<Mutex<HashSet<String>>>) {
    let resolved = Arc::new(Mutex::default());
    (
        Value::from_object(TrackedContext {
            enclosed: ctx,
            resolved: resolved.clone(),
        }),
        resolved,
    )
}

static TEMPLATE: &str = r#"
{%- set locally_set = 'a-value' -%}
name={{ name }}
undefined_value={{ undefined_value }}
global={{ global }}
locally_set={{ locally_set }}
"#;

fn main() {
    let mut env = Environment::new();
    env.add_global("global", true);
    let template = env.template_from_str(TEMPLATE).unwrap();

    let (ctx, resolved) = track_context(context! {
        name => "John",
        unused => 42
    });

    println!("{}", template.render(ctx).unwrap());
    println!("resolved: {:?}", resolved.lock().unwrap());
}
