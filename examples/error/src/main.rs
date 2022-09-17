use minijinja::{context, Environment};

fn main() {
    let mut env = Environment::new();
    env.set_debug(true);
    if let Err(err) = env.add_template(
        "hello.txt",
        r#"
        first line
        {% for item in seq %}
          {% with item_squared = item * item %}
            {% with foo = 42 %}
              {{ range(10) }}
              {{ other_seq|join(" ") }}
              Hello {{ item_squared + bar }}!
            {% endwith %}
          {% endwith %}
        {% endfor %}
        last line
        "#,
    ) {
        eprintln!("Template Failed Parsing:");
        eprintln!("  {:#}", err);
        std::process::exit(1);
    }
    let template = env.get_template("hello.txt").unwrap();
    let ctx = context! {
        seq => vec![2, 4, 8],
        other_seq => (0..5).collect::<Vec<_>>(),
        bar => "test"
    };
    match template.render(&ctx) {
        Ok(result) => println!("{}", result),
        Err(err) => {
            eprintln!("Template Failed Rendering:");
            eprintln!("  {:#}", err);
            std::process::exit(1);
        }
    }
}
