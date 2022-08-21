use minijinja::{Environment, Source};
use once_cell::sync::Lazy;
use std::collections::BTreeMap;

static ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    let mut source = Source::new();
    source.load_from_path("templates", &["txt"]).unwrap();
    env.set_source(source);
    env
});

fn main() {
    let mut ctx = BTreeMap::new();
    ctx.insert("name", "World");
    let tmpl = ENV.get_template("hello.txt").unwrap();
    println!("{}", tmpl.render(&ctx).unwrap());
}
