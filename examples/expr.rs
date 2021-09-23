use minijinja::Environment;
use serde::Serialize;

#[derive(Serialize)]
pub struct Context {
    foo: usize,
}

fn main() {
    let env = Environment::new();
    let expr = env.compile_expression("foo == 42").unwrap();
    let result = expr.eval(&Context { foo: 42 }).unwrap();
    println!("result: {:?}", result);
}
