use std::path::PathBuf;

use minijinja::value::{Value, ViaDeserialize};
use minijinja::{context, Environment};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Point {
    x: f32,
    y: f32,
}

fn dirname(path: ViaDeserialize<PathBuf>) -> String {
    match path.parent() {
        Some(parent) => parent.display().to_string(),
        None => "".to_string(),
    }
}

fn main() {
    let mut env = Environment::new();
    env.add_template("path.txt", include_str!("path.txt"))
        .unwrap();
    env.add_filter("dirname", dirname);

    // First example: shows ViaDeserialize
    let template = env.get_template("path.txt").unwrap();
    println!(
        "{}",
        template
            .render(context! {
                path => std::env::current_dir().unwrap(),
            })
            .unwrap()
    );

    // Second example shows how you can deserialize directly from a value
    let point = Point { x: -1.0, y: 1.0 };
    let point_value = Value::from_serializable(&point);
    println!("Point serialized as value: {}", point_value);
    let point_again = Point::deserialize(point_value).unwrap();
    println!("Point deserialization: {:?}", point_again);
}
