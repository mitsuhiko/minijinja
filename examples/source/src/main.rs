use minijinja::{context, Environment, Source};
use once_cell::sync::Lazy;

static ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    env.set_source(Source::from_path("templates"));
    env
});

fn main() {
    let tmpl = ENV.get_template("hello.txt").unwrap();
    let ctx = context!(name => "World");
    println!("{}", tmpl.render(ctx).unwrap());
}
