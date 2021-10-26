use minijinja::{context, Environment};
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    name: String,
}

fn main() {
    let mut env = Environment::new();
    env.add_template("hello.txt", "Hello {{ user.name }}!")
        .unwrap();
    let template = env.get_template("hello.txt").unwrap();
    let user = User {
        name: "John".into(),
    };
    println!("{}", template.render(context!(user)).unwrap());
}
