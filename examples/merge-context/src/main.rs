use minijinja::{context, Environment};

fn main() {
    let env = Environment::new();
    let ctx = context! { a => "A", ..context! { b => "B" } };
    println!(
        "{}",
        env.render_str("Two variables: {{ a }} and {{ b }}!", ctx)
            .unwrap()
    );
}
