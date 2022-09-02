use std::collections::BTreeMap;
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
        #[cfg(feature = "debug")]
        {
            env.set_debug(false);
        }
        let ctx: serde_json::Value = serde_json::from_str(iter.next().unwrap()).unwrap();

        for (path, source) in &refs {
            let ref_filename = path.file_name().unwrap().to_str().unwrap();
            env.add_template(ref_filename, source).unwrap();
        }

        let content = iter.next().unwrap();
        env.add_template(filename, content).unwrap();
        let template = env.get_template(filename).unwrap();
        dbg!(&template);

        let mut rendered = match template.render(&ctx) {
            Ok(rendered) => rendered,
            Err(err) => format!("!!!ERROR!!!\n\n{:?}\n", err),
        };
        rendered.push('\n');

        insta::with_settings!({
            info => &context! { ctx },
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
