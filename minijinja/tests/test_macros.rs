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
    fn type_name_of_val<T: ?Sized>(_val: &T) -> &str {
        std::any::type_name::<T>()
    }

    let args = args!();
    assert_eq!(args.len(), 0);
    assert_eq!(type_name_of_val(args), "[minijinja::value::Value]");

    let args = args!(1, 2);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));
    assert_eq!(type_name_of_val(args), "[minijinja::value::Value]");

    let args = args!(1, 2,);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));

    let args = args!(1, 2, foo => 42, bar => 23);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));
    let kwargs = Kwargs::try_from(args[2].clone()).unwrap();
    assert_eq!(kwargs.get::<i32>("foo").unwrap(), 42);
    assert_eq!(kwargs.get::<i32>("bar").unwrap(), 23);

    let args = args!(1, 2, foo => 42, bar => 23,);
    assert_eq!(args[0], Value::from(1));
    assert_eq!(args[1], Value::from(2));
    let kwargs = Kwargs::try_from(args[2].clone()).unwrap();
    assert_eq!(kwargs.get::<i32>("foo").unwrap(), 42);
    assert_eq!(kwargs.get::<i32>("bar").unwrap(), 23);
    assert_eq!(type_name_of_val(args), "[minijinja::value::Value]");
}
