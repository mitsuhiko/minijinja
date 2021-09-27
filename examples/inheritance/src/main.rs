use minijinja::Environment;
use serde::Serialize;

#[derive(Serialize)]
pub struct Page {
    title: String,
    content: String,
}

#[derive(Serialize)]
pub struct Context {
    page: Page,
}

fn main() {
    let mut env = Environment::new();
    env.add_template("layout.html", include_str!("layout.html"))
        .unwrap();
    env.add_template("index.html", include_str!("index.html"))
        .unwrap();

    let template = env.get_template("index.html").unwrap();
    let ctx = &Context {
        page: Page {
            title: "Some title".into(),
            content: "Lorum Ipsum".into(),
        },
    };
    println!("{}", template.render(&ctx).unwrap());
}
