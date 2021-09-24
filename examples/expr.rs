use minijinja::Environment;
use std::env;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let env = Environment::new();
    let expr = env.compile_expression(&args[1]).unwrap();
    let result = expr.eval(&()).unwrap();
    let serialized = serde_json::to_string_pretty(&result).unwrap();
    println!("{}", serialized);
}
