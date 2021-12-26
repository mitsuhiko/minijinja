use minijinja::{context, Environment};

fn main() {
    let mut env = Environment::new();
    env.set_debug(true);
    env.add_template(
        "hello.txt",
        r#"
        first line
        {% for item in seq %}
          {% with item_squared = item * item %}
            Hello {{ item_squared + bar }}!
          {% endwith %}
        {% endfor %}
        last line
        "#,
    )
    .unwrap();
    let template = env.get_template("hello.txt").unwrap();
    let ctx = context! {
        seq => vec![2, 4, 8],
        bar => "test"
    };
    match template.render(&ctx) {
        Ok(result) => println!("{}", result),
        Err(err) => {
            eprintln!("Template Failed Rendering:");
            eprintln!("  {:#}", err);
        }
    }
}
