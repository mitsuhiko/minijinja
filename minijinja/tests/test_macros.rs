use similar_asserts::assert_eq;

use minijinja::value::{Kwargs, Value};
use minijinja::{args, context, render, Environment};

#[test]
fn test_context() {
    let var1 = 23;
    let ctx = context!(var1, var2 => 42);
    assert_eq!(ctx.get_attr("var1").unwrap(), Value::from(23));
    assert_eq!(ctx.get_attr("var2").unwrap(), Value::from(42));
}

#[test]
fn test_render() {
    let env = Environment::new();
    let rv = render!(in env, "Hello {{ name }}!", name => "World");
    assert_eq!(rv, "Hello World!");

    let rv = render!("Hello {{ name }}!", name => "World");
    assert_eq!(rv, "Hello World!");

    let rv = render!("Hello World!");
    assert_eq!(rv, "Hello World!");
}

#[test]
fn test_args() {
    let args = args!(1, 2);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));

    let args = args!(1, 2, foo => 42, bar => 23);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));
    let kwargs = Kwargs::try_from(args[2].clone()).unwrap();
    assert_eq!(kwargs.get::<i32>("foo").unwrap(), 42);
    assert_eq!(kwargs.get::<i32>("bar").unwrap(), 23);
}
