use minijinja::value::Value;
use minijinja::{Environment, UndefinedBehavior};

#[test]
fn test_basics() {
    let env = Environment::new();
    assert_eq!(env.undefined_behavior(), UndefinedBehavior::Lenient);

    assert_eq!(
        env.compile_expression("true.undefined")
            .unwrap()
            .eval(())
            .unwrap(),
        Value::UNDEFINED
    );

    assert_eq!(
        env.compile_expression("undefined|list")
            .unwrap()
            .eval(())
            .unwrap(),
        Value::from(Vec::<Value>::new())
    );

    assert_eq!(
        env.render_str("<{% for x in undefined %}...{% endfor %}>", ())
            .unwrap(),
        "<>"
    );
}
