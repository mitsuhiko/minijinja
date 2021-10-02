#![cfg(feature = "source")]

use minijinja::{Environment, Source};

fn create_env() -> Environment<'static> {
    let mut source = Source::new();
    let template = String::from("Hello World!");
    source.add_template("hello", template).unwrap();
    let mut env = Environment::new();
    env.set_source(source);
    env
}

#[test]
fn test_basic() {
    let env = create_env();
    let t = env.get_template("hello").unwrap();
    assert_eq!(t.render(&()).unwrap(), "Hello World!");
}
