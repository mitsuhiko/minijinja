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

#[test]
fn test_dynamic() {
    let mut source = Source::new();
    source.set_loader(|name| match name {
        "hello" => Ok(Some("Hello World!".into())),
        _ => Ok(None),
    });
    let template = String::from("Hello World!");
    source.add_template("hello", template).unwrap();
    let mut env = Environment::new();
    env.set_source(source);
    let t = env.get_template("hello").unwrap();
    assert_eq!(t.render(&()).unwrap(), "Hello World!");
    let err = env.get_template("missing").unwrap_err();
    assert_eq!(
        err.to_string(),
        "template not found: template \"missing\" does not exist"
    );
}
