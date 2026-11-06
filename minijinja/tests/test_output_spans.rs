#![cfg(all(feature = "debug", feature = "builtins"))]
use std::fmt::Write;

use minijinja::syntax::SyntaxConfig;
use minijinja::value::Value;
use minijinja::{context, Environment, OutputSpanKind};

/// Renders a template and returns a description of the recorded spans.
///
/// This also checks that the spans cover the entire output without gaps
/// and that raw spans point to the exact source that was written.
fn render_spans(env: &Environment, name: &str, ctx: Value) -> String {
    let tmpl = env.get_template(name).unwrap();
    let (output, spans) = tmpl.render_with_spans(ctx.clone()).unwrap();
    assert_eq!(output, tmpl.render(ctx).unwrap());

    let mut pos = 0;
    let mut rv = String::new();
    for span in &spans {
        let output_range = span.output_range();
        assert_eq!(output_range.start, pos, "gap or overlap in output spans");
        assert!(output_range.end > output_range.start, "empty output span");
        pos = output_range.end;

        let source = env.get_template(span.name()).unwrap().source().to_string();
        let source_text = &source[span.source_range()];
        let output_text = &output[output_range];
        let line = source[..span.source_range().start].matches('\n').count() + 1;
        assert_eq!(span.line(), line, "line does not match source range");
        if span.kind() == OutputSpanKind::Raw {
            assert_eq!(output_text, source_text);
        }
        writeln!(
            rv,
            "{}:{} {:?} {:?} <- {:?}",
            span.name(),
            span.line(),
            span.kind(),
            output_text,
            source_text
        )
        .unwrap();
    }
    assert_eq!(pos, output.len(), "output not fully covered by spans");
    rv
}

#[test]
fn test_basic() {
    let mut env = Environment::new();
    env.add_template(
        "t.txt",
        "Hello {{ name }}!\n{% for item in items %}- {{ item|upper }}\n{% endfor %}{{ empty }}{{\n  name\n}}",
    )
    .unwrap();
    insta::assert_snapshot!(render_spans(&env, "t.txt", context!(name => "World", items => vec!["a", "b"], empty => "")), @r#"
    t.txt:1 Raw "Hello " <- "Hello "
    t.txt:1 Expression "World" <- "name"
    t.txt:1 Raw "!\n" <- "!\n"
    t.txt:2 Raw "- " <- "- "
    t.txt:2 Expression "A" <- "item|upper"
    t.txt:2 Raw "\n" <- "\n"
    t.txt:2 Raw "- " <- "- "
    t.txt:2 Expression "B" <- "item|upper"
    t.txt:2 Raw "\n" <- "\n"
    t.txt:4 Expression "World" <- "name"
    "#);
}

#[test]
fn test_whitespace_control() {
    let mut env = Environment::new();
    env.set_syntax(
        SyntaxConfig::builder()
            .trim_blocks(true)
            .lstrip_blocks(true)
            .build()
            .unwrap(),
    );
    env.add_template(
        "t.txt",
        "<ul>\n  {% for x in seq %}\n  <li>{{- x -}}  </li>\n  {% endfor %}\n</ul>\n{% raw %}\n  {{ raw }}  {%- endraw %}!",
    )
    .unwrap();
    insta::assert_snapshot!(render_spans(&env, "t.txt", context!(seq => vec![1, 2])), @r#"
    t.txt:1 Raw "<ul>\n" <- "<ul>\n"
    t.txt:3 Raw "  <li>" <- "  <li>"
    t.txt:3 Expression "1" <- "x"
    t.txt:3 Raw "</li>\n" <- "</li>\n"
    t.txt:3 Raw "  <li>" <- "  <li>"
    t.txt:3 Expression "2" <- "x"
    t.txt:3 Raw "</li>\n" <- "</li>\n"
    t.txt:5 Raw "</ul>\n" <- "</ul>\n"
    t.txt:7 Raw "  {{ raw }}" <- "  {{ raw }}"
    t.txt:7 Raw "!" <- "!"
    "#);
}

#[cfg(feature = "macros")]
#[test]
fn test_captured_output() {
    let mut env = Environment::new();
    env.add_template(
        "t.txt",
        "{% macro m(x) %}[{{ x }}{{ caller() if caller }}]{% endmacro %}\
         {{ m(1) }}|{% call m(2) %}c{% endcall %}|\
         {% filter upper|trim %} a{{ 'b' }} {% endfilter %}|\
         {% set x %}captured{% endset %}{{ x }}",
    )
    .unwrap();
    insta::assert_snapshot!(render_spans(&env, "t.txt", context!()), @r#"
    t.txt:1 Expression "[1]" <- "m(1)"
    t.txt:1 Raw "|" <- "|"
    t.txt:1 Expression "[2c]" <- "m(2)"
    t.txt:1 Raw "|" <- "|"
    t.txt:1 Expression "AB" <- "upper|trim"
    t.txt:1 Raw "|" <- "|"
    t.txt:1 Expression "captured" <- "x"
    "#);
}

#[test]
fn test_recursive_loop() {
    let mut env = Environment::new();
    env.add_template(
        "t.txt",
        "{% for item in tree recursive %}({{ item.name }}{{ loop(item.children) }}){% endfor %}",
    )
    .unwrap();
    let tree = context!(tree => vec![
        context!(name => "a", children => vec![context!(name => "b", children => Vec::<Value>::new())]),
    ]);
    insta::assert_snapshot!(render_spans(&env, "t.txt", tree), @r#"
    t.txt:1 Raw "(" <- "("
    t.txt:1 Expression "a" <- "item.name"
    t.txt:1 Raw "(" <- "("
    t.txt:1 Expression "b" <- "item.name"
    t.txt:1 Raw ")" <- ")"
    t.txt:1 Raw ")" <- ")"
    "#);
}

#[cfg(feature = "multi_template")]
#[test]
fn test_multi_template() {
    let mut env = Environment::new();
    env.add_template(
        "layout.html",
        "<title>{% block title %}Default{% endblock %}</title>\n{% block body %}{% endblock %}",
    )
    .unwrap();
    env.add_template("item.html", "<li>{{ item }}</li>")
        .unwrap();
    env.add_template(
        "page.html",
        "{% extends \"layout.html\" %}ignored\
         {% block title %}{{ title }} - {{ super() }}{% endblock %}\
         {% block body %}<ul>{% for item in items %}{% include \"item.html\" %}{% endfor %}</ul>{% endblock %}",
    )
    .unwrap();
    insta::assert_snapshot!(render_spans(&env, "page.html", context!(title => "Page", items => vec![1, 2])), @r#"
    layout.html:1 Raw "<title>" <- "<title>"
    page.html:1 Expression "Page" <- "title"
    page.html:1 Raw " - " <- " - "
    layout.html:1 Raw "Default" <- "Default"
    layout.html:1 Raw "</title>\n" <- "</title>\n"
    page.html:1 Raw "<ul>" <- "<ul>"
    item.html:1 Raw "<li>" <- "<li>"
    item.html:1 Expression "1" <- "item"
    item.html:1 Raw "</li>" <- "</li>"
    item.html:1 Raw "<li>" <- "<li>"
    item.html:1 Expression "2" <- "item"
    item.html:1 Raw "</li>" <- "</li>"
    page.html:1 Raw "</ul>" <- "</ul>"
    "#);
}

#[cfg(feature = "multi_template")]
#[test]
fn test_recursive_loop_from_include() {
    let mut env = Environment::new();
    env.add_template("i", "|{{ loop(x[1:] if x is sequence else []) }}")
        .unwrap();
    env.add_template(
        "t",
        "{% for x in [[1,[2]]] recursive %}<{{ x }}{% include 'i' %}>{% endfor %}!",
    )
    .unwrap();
    insta::assert_snapshot!(render_spans(&env, "t", context!()), @r#"
    t:1 Raw "<" <- "<"
    t:1 Expression "[1, [2]]" <- "x"
    i:1 Raw "|" <- "|"
    t:1 Raw "<" <- "<"
    t:1 Expression "[2]" <- "x"
    i:1 Raw "|" <- "|"
    t:1 Raw ">" <- ">"
    t:1 Raw ">" <- ">"
    t:1 Raw "!" <- "!"
    "#);
}

#[cfg(feature = "multi_template")]
#[test]
fn test_nested_extends_and_self() {
    let mut env = Environment::new();
    env.add_template(
        "base.txt",
        "[{% block a %}base{% endblock %}]{{ self.a() }}",
    )
    .unwrap();
    env.add_template(
        "middle.txt",
        "{% extends 'base.txt' %}{% block a %}middle {{ super() }}{% endblock %}",
    )
    .unwrap();
    env.add_template(
        "child.txt",
        "{% extends 'middle.txt' %}{% block a %}child {{ super() }}{% endblock %}",
    )
    .unwrap();
    insta::assert_snapshot!(render_spans(&env, "child.txt", context!()), @r#"
    base.txt:1 Raw "[" <- "["
    child.txt:1 Raw "child " <- "child "
    middle.txt:1 Raw "middle " <- "middle "
    base.txt:1 Raw "base" <- "base"
    base.txt:1 Raw "]" <- "]"
    child.txt:1 Raw "child " <- "child "
    middle.txt:1 Raw "middle " <- "middle "
    base.txt:1 Raw "base" <- "base"
    "#);
}

#[test]
fn test_custom_formatter_and_escaping() {
    let mut env = Environment::new();
    env.add_template("t.html", "<p>{{ value }}</p>").unwrap();
    insta::assert_snapshot!(render_spans(&env, "t.html", context!(value => "<b>")), @r#"
    t.html:1 Raw "<p>" <- "<p>"
    t.html:1 Expression "&lt;b&gt;" <- "value"
    t.html:1 Raw "</p>" <- "</p>"
    "#);

    env.set_formatter(|out, _state, value| write!(out, "<{value}>").map_err(Into::into));
    insta::assert_snapshot!(render_spans(&env, "t.html", context!(value => 42)), @r#"
    t.html:1 Raw "<p>" <- "<p>"
    t.html:1 Expression "<42>" <- "value"
    t.html:1 Raw "</p>" <- "</p>"
    "#);
}

#[test]
fn test_error() {
    let env = Environment::new();
    let tmpl = env.template_from_str("a{{ 1 / 0 }}").unwrap();
    assert!(tmpl.render_with_spans(()).is_err());
}
