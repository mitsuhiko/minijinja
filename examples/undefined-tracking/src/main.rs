use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use minijinja::value::{StructObject, Value, ValueKind};
use minijinja::{context, Environment};

struct TrackedContext {
    enclosed: Value,
    undefined: Arc<Mutex<HashSet<String>>>,
}

impl StructObject for TrackedContext {
    fn get_field(&self, name: &str) -> Option<Value> {
        match self
            .enclosed
            .get_attr(name)
            .ok()
            .filter(|x| !x.is_undefined())
        {
            Some(rv) => Some(rv),
            None => {
                let mut undefined = self.undefined.lock().unwrap();
                if !undefined.contains(name) {
                    undefined.insert(name.to_string());
                }
                None
            }
        }
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
    let undefined = Arc::new(Mutex::default());
    (
        Value::from_struct_object(TrackedContext {
            enclosed: ctx,
            undefined: undefined.clone(),
        }),
        undefined,
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

    let (ctx, undefined) = track_context(context! {
        name => "John",
        unused => 42
    });

    let (rv, state) = template.render_and_return_state(ctx).unwrap();
    println!("{}", rv);

    // we need to make a copy here to not deadlock when we try to lookup
    // on the state later.
    let all_undefined = undefined.lock().unwrap().clone();

    // easy case: undefined contains all values not looked up in the context
    println!("not found in context: {:?}", all_undefined);

    // to filter out globals we need to make another lookup:
    let undefined = all_undefined
        .iter()
        .filter(|x| state.lookup(x).is_none())
        .collect::<HashSet<_>>();
    println!("completely undefined: {:?}", undefined);
}
