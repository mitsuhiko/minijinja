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

const LAYOUT_TEMPLATE: &str = r#"<!doctype html>
<html>
  <head><title>{% block title %}some website{% endblock %}</title></head>
  <body>{% block body %}{% endblock %}</body>
</html>"#;
const INDEX_TEMPLATE: &str = r#"{% extends "base.html" %}
{% block title %}{{ page.title|upper }} | {{ super() }}{% endblock %}
{% block body %}{{ page.content }}{% endblock %}"#;

fn main() {
    let mut env = Environment::new();
    env.add_template("base.html", LAYOUT_TEMPLATE).unwrap();
    env.add_template("index.html", INDEX_TEMPLATE).unwrap();

    let template = env.get_template("index.html").unwrap();
    let ctx = &Context {
        page: Page {
            title: "Some title".into(),
            content: "Lorum Ipsum".into(),
        },
    };
    println!("{}", template.render(&ctx).unwrap());
}
