use minijinja::{context, Environment, Source};
use once_cell::sync::Lazy;

static ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    let mut source = Source::new();
    source.load_from_path("templates", &["txt"]).unwrap();
    env.set_source(source);
    env
});

fn main() {
    let tmpl = ENV.get_template("hello.txt").unwrap();
    let ctx = context!(name => "World");
    println!("{}", tmpl.render(ctx).unwrap());
}
