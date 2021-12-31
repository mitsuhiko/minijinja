use minijinja::{context, Environment};

fn main() {
    let mut env = Environment::new();
    env.add_template("hello.txt", "Hello {{ name }}!").unwrap();
    let tmpl = env.get_template("hello.txt").unwrap();
    println!("{}", tmpl.render(context!(name => "World")).unwrap());
}
