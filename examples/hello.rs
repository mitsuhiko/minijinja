use minijinja::Environment;
use serde::Serialize;

#[derive(Serialize)]
pub struct User {
    name: String,
}

#[derive(Serialize)]
pub struct Context {
    user: User,
}

fn main() {
    let mut env = Environment::new();
    env.add_template("hello.txt", "Hello {{ user.name }}!").unwrap();
    let template = env.get_template("hello.txt").unwrap();
    println!("{}", template.render(&Context {
        user: User {
            name: "John".into(),
        },
    }).unwrap());
}

