use std::collections::BTreeMap;
use minijinja::{Environment, Source};
use lazy_static::lazy_static;

lazy_static! {
    static ref ENV: Environment<'static> = create_env();
}

fn create_env() -> Environment<'static> {
    let mut env = Environment::new();
    let mut source = Source::new();
    source.add_template("hello.txt", "Hello {{ name }}!").unwrap();
    env.set_source(source);
    env
}

fn main() {
    let mut ctx = BTreeMap::new();
    ctx.insert("name", "World");
    let tmpl = ENV.get_template("hello.txt").unwrap();
    println!("{}", tmpl.render(&ctx).unwrap());
}
