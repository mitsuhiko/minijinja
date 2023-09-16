use std::sync::Arc;

use minijinja::value::{StructObject, Value, ValueKind};
use minijinja::{context, Environment};

struct SelfReferentialContext {
    ctx: Value,
}

impl StructObject for SelfReferentialContext {
    fn get_field(&self, name: &str) -> Option<Value> {
        if name == "CONTEXT" {
            return Some(self.ctx.clone());
        }
        self.ctx.get_attr(name).ok().filter(|x| !x.is_undefined())
    }

    fn fields(&self) -> Vec<Arc<str>> {
        if self.ctx.kind() == ValueKind::Map {
            if let Ok(keys) = self.ctx.try_iter() {
                return keys.filter_map(|x| Arc::<str>::try_from(x).ok()).collect();
            }
        }
        Vec::new()
    }
}

pub fn make_self_referential(ctx: Value) -> Value {
    Value::from_struct_object(SelfReferentialContext { ctx })
}

static TEMPLATE: &str = r#"
name: {{ name }}
CONTEXT.name: {{ CONTEXT.name }}
CONTEXT.CONTEXT is undefined: {{ CONTEXT.CONTEXT is undefined }}
CONTEXT: {{ CONTEXT }}
"#;

fn main() {
    let env = Environment::new();
    let template = env.template_from_str(TEMPLATE).unwrap();

    let ctx = make_self_referential(context! {
        name => "John",
        other_value => 42,
    });

    println!("{}", template.render(ctx).unwrap());
}
