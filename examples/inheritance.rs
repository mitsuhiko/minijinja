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
    env.add_template(
        "base.html",
        r#"<html>
    <head><title>{% block title %}some website{% endblock %}</title></head>
    <body>{% block body %}{% endblock %}</body></html>"#,
    )
    .unwrap();
    env.add_template(
        "index.html",
        r#"{% extends "base.html" %}
{% block title %}{{ page.title|upper }} | {{ super() }}{% endblock %}
{% block body %}{{ page.content }}{% endblock %}"#,
    )
    .unwrap();
    let template = env.get_template("index.html").unwrap();
    println!(
        "{}",
        template
            .render(&Context {
                page: Page {
                    title: "Some title".into(),
                    content: "Lorum Ipsum".into(),
                },
            })
            .unwrap()
    );
}
