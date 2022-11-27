use std::collections::BTreeMap;

use similar_asserts::assert_eq;

use minijinja::value::Value;
use minijinja::Environment;

#[test]
fn test_basic() {
    let mut env = Environment::new();
    env.add_template("test", "{% for x in seq %}[{{ x }}]{% endfor %}")
        .unwrap();
    let t = env.get_template("test").unwrap();
    let mut ctx = BTreeMap::new();
    ctx.insert("seq", Value::from((0..3).collect::<Vec<_>>()));
    let rv = t.render(ctx).unwrap();
    assert_eq!(rv, "[0][1][2]");
}

#[test]
fn test_expression() {
    let env = Environment::new();
    let expr = env.compile_expression("foo + bar").unwrap();
    let mut ctx = BTreeMap::new();
    ctx.insert("foo", 42);
    ctx.insert("bar", 23);
    assert_eq!(expr.eval(&ctx).unwrap(), Value::from(65));
}

#[test]
fn test_expression_lifetimes() {
    let mut env = Environment::new();
    let s = String::new();
    env.add_template("test", &s).unwrap();
    {
        let x = String::from("1 + 1");
        let expr = env.compile_expression(&x).unwrap();
        assert_eq!(expr.eval(()).unwrap().to_string(), "2");
    }
}

#[test]
fn test_clone() {
    let mut env = Environment::new();
    env.add_template("test", "a").unwrap();
    let mut env2 = env.clone();
    assert_eq!(env2.get_template("test").unwrap().render(()).unwrap(), "a");
    env2.add_template("test", "b").unwrap();
    assert_eq!(env2.get_template("test").unwrap().render(()).unwrap(), "b");
    assert_eq!(env.get_template("test").unwrap().render(()).unwrap(), "a");
}

#[test]
fn test_globals() {
    let mut env = Environment::new();
    env.add_global("a", Value::from(42));
    env.add_template("test", "{{ a }}").unwrap();
    let tmpl = env.get_template("test").unwrap();
    assert_eq!(tmpl.render(()).unwrap(), "42");
}

#[test]
fn test_template_removal() {
    let mut env = Environment::new();
    env.add_template("test", "{{ a }}").unwrap();
    env.remove_template("test");
    assert!(env.get_template("test").is_err());
}
