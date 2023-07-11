//! This example dumps out YAML by using auto escaping.
use std::collections::BTreeMap;
use std::env;

use minijinja::{context, Environment, Syntax};

fn main() {
    let mut env = Environment::new();
    env.set_syntax(Syntax {
        block_start: "{%".into(),
        block_end: "%}".into(),
        variable_start: "${{".into(),
        variable_end: "}}".into(),
        comment_start: "{#".into(),
        comment_end: "#}".into(),
    })
    .unwrap();
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
