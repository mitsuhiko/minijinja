use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;

use minijinja::{context, Environment, Error, State};

use similar_asserts::assert_eq;

#[test]
fn test_vm() {
    let mut refs = Vec::new();
    for entry in fs::read_dir("tests/inputs/refs").unwrap() {
        let entry = entry.unwrap();
        let filename = entry.file_name();
        let filename = filename.to_str().unwrap();
        if !filename.ends_with(".txt") && !filename.ends_with(".html") {
            continue;
        }
        let source = fs::read_to_string(entry.path()).unwrap();
        refs.push((entry.path().clone(), source));
    }

    insta::glob!("inputs/*", |path| {
        if !path.metadata().unwrap().is_file() {
            return;
        }
        let filename = path.file_name().unwrap().to_str().unwrap();
        let contents = std::fs::read_to_string(path).unwrap();
        let mut iter = contents.splitn(2, "\n---\n");
        let mut env = Environment::new();
        let ctx: serde_json::Value = serde_json::from_str(iter.next().unwrap()).unwrap();

        for (path, source) in &refs {
            let ref_filename = path.file_name().unwrap().to_str().unwrap();
            env.add_template(ref_filename, source).unwrap();
        }

        let content = iter.next().unwrap();
        let rendered = if let Err(err) = env.add_template(filename, content) {
            let mut rendered = format!("!!!SYNTAX ERROR!!!\n\n{:#?}\n\n", err);
            writeln!(rendered, "{:#}", err).unwrap();
            rendered
        } else {
            let template = env.get_template(filename).unwrap();

            match template.render(&ctx) {
                Ok(mut rendered) => {
                    rendered.push('\n');
                    rendered
                }
                Err(err) => {
                    let mut rendered = format!("!!!ERROR!!!\n\n{:#?}\n\n", err);

                    writeln!(rendered, "{:#}", err).unwrap();
                    let mut err = &err as &dyn std::error::Error;
                    while let Some(next_err) = err.source() {
                        writeln!(rendered).unwrap();
                        writeln!(rendered, "caused by: {:#}", next_err).unwrap();
                        err = next_err;
                    }

                    rendered
                }
            }
        };

        insta::with_settings!({
            info => &ctx,
            description => content.trim_end(),
            omit_expression => true
        }, {
            insta::assert_snapshot!(&rendered);
        });
    });
}

#[test]
fn test_custom_filter() {
    fn test_filter(_: &State, value: String) -> Result<String, Error> {
        Ok(format!("[{}]", value))
    }

    let mut ctx = BTreeMap::new();
    ctx.insert("var", 42);

    let mut env = Environment::new();
    env.add_filter("test", test_filter);
    env.add_template("test", "{{ var|test }}").unwrap();
    let tmpl = env.get_template("test").unwrap();
    let rv = tmpl.render(&ctx).unwrap();
    assert_eq!(rv, "[42]");
}

#[test]
fn test_single() {
    let mut env = Environment::new();
    env.add_template("simple", "Hello {{ name }}!").unwrap();
    let tmpl = env.get_template("simple").unwrap();
    let rv = tmpl.render(context!(name => "Peter")).unwrap();
    assert_eq!(rv, "Hello Peter!");
}

#[test]
fn test_auto_escaping() {
    let mut env = Environment::new();
    env.add_template("index.html", "{{ var }}").unwrap();
    #[cfg(feature = "json")]
    {
        env.add_template("index.js", "{{ var }}").unwrap();
    }
    env.add_template("index.txt", "{{ var }}").unwrap();

    // html
    let tmpl = env.get_template("index.html").unwrap();
    let rv = tmpl.render(context!(var => "<script>")).unwrap();
    insta::assert_snapshot!(rv, @"&lt;script&gt;");

    // JSON
    #[cfg(feature = "json")]
    {
        use minijinja::value::Value;
        let tmpl = env.get_template("index.js").unwrap();
        let rv = tmpl.render(context!(var => "foo\"bar'baz")).unwrap();
        insta::assert_snapshot!(rv, @r###""foo\"bar'baz""###);
        let rv = tmpl
            .render(context!(var => [Value::from(true), Value::from("<foo>"), Value::from(())]))
            .unwrap();
        insta::assert_snapshot!(rv, @r###"[true,"<foo>",null]"###);
    }

    // Text
    let tmpl = env.get_template("index.txt").unwrap();
    let rv = tmpl.render(context!(var => "foo\"bar'baz")).unwrap();
    insta::assert_snapshot!(rv, @r###"foo"bar'baz"###);
}

#[test]
fn test_loop_changed() {
    let rv = minijinja::render!(
        r#"
        {%- for i in items -%}
          {% if loop.changed(i) %}{{ i }}{% endif %}
        {%- endfor -%}
        "#,
        items => vec![1, 1, 1, 2, 3, 4, 4, 5],
    );
    assert_eq!(rv, "12345");
}

#[test]
fn test_current_call_state() {
    use minijinja::value::{Object, Value};
    use std::fmt;

    #[derive(Debug)]
    struct MethodAndFunc;

    impl fmt::Display for MethodAndFunc {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{self:?}")
        }
    }

    impl Object for MethodAndFunc {
        fn call_method(&self, state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
            assert_eq!(name, state.current_call().unwrap());
            let args = args
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            Ok(format!("{}({args})", state.current_call().unwrap()).into())
        }

        fn call(&self, state: &State, args: &[Value]) -> Result<Value, Error> {
            let args = args
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            Ok(format!("{}({args})", state.current_call().unwrap()).into())
        }
    }

    fn current_call(state: &State, value: Option<&str>) -> String {
        format!("{}({})", state.current_call().unwrap(), value.unwrap_or(""))
    }

    fn check_test(state: &State, value: &str) -> bool {
        state.current_call() == Some(value)
    }

    let mut env = Environment::new();
    env.add_function("fn_call_a", current_call);
    env.add_function("fn_call_b", current_call);
    env.add_filter("filter_call", current_call);
    env.add_test("my_test", check_test);
    env.add_test("another_test", check_test);
    env.add_global("object", Value::from_object(MethodAndFunc));

    env.add_template(
        "test",
        r#"
        {{ fn_call_a() }}
        {{ "foo" | filter_call }}
        {{ fn_call_a() | filter_call }}
        {{ fn_call_b() | filter_call }}
        {{ fn_call_a(fn_call_b()) }}
        {{ fn_call_a(fn_call_b()) | filter_call }}

        {{ "my_test" is my_test }}
        {{ "another_test" is my_test }}
        {{ "another_test" is another_test }}

        {{ object.foo() }}
        {{ object.bar() }}
        {{ object.foo(object.bar(object.baz())) }}
        {{ object(object.bar()) }}
        {{ object.baz(object()) }}
    "#,
    )
    .unwrap();

    let tmpl = env.get_template("test").unwrap();
    let rv = tmpl.render(context!()).unwrap();
    assert_eq!(
        rv,
        r#"
        fn_call_a()
        filter_call(foo)
        filter_call(fn_call_a())
        filter_call(fn_call_b())
        fn_call_a(fn_call_b())
        filter_call(fn_call_a(fn_call_b()))

        true
        false
        true

        foo()
        bar()
        foo(bar(baz()))
        object(bar())
        baz(object())
    "#
    );
}
