//! This example dumps out YAML by using auto escaping.
use std::collections::BTreeMap;
use std::env;

use minijinja::{context, Environment};

fn main() {
    let mut env = Environment::new();
    env.add_template("template.yml", include_str!("template.yaml"))
        .unwrap();
    let tmpl = env.get_template("template.yml").unwrap();
    println!(
        "{}",
        tmpl.render(context! {
            env => env::vars().collect::<BTreeMap<_, _>>(),
            title => "Hello World!",
            yaml => "[1, 2, 3]",
        })
        .unwrap()
    );
}
