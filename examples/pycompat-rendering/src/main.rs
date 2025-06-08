use minijinja::{context, Environment};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MiniJinja Rendering Comparison ===");

    // Create environments for both modes to avoid borrowing issues
    let mut env_default = Environment::new();
    let mut env_pycompat = Environment::new();

    // Add the same template to both environments
    let template_str = r#"{{ [true, false, none, 'foo', "bar'baz", '\x13'] }}"#;
    env_default.add_template("demo", template_str)?;
    env_pycompat.add_template("demo", template_str)?;

    // Configure rendering modes
    env_default.set_pycompat_rendering(false);
    env_pycompat.set_pycompat_rendering(true);

    // Test rendering
    let result_default = env_default.get_template("demo")?.render(context! {})?;
    let result_pycompat = env_pycompat.get_template("demo")?.render(context! {})?;

    println!("Default rendering: {}", result_default);
    println!("PyCompat rendering: {}", result_pycompat);

    println!("\n=== Individual Value Comparison ===");

    // Test individual values
    env_default.add_template("bool_true", "{{ true }}")?;
    env_default.add_template("bool_false", "{{ false }}")?;
    env_default.add_template("none_val", "{{ none }}")?;

    env_pycompat.add_template("bool_true", "{{ true }}")?;
    env_pycompat.add_template("bool_false", "{{ false }}")?;
    env_pycompat.add_template("none_val", "{{ none }}")?;

    println!("Default mode:");
    println!(
        "  true -> {}",
        env_default.get_template("bool_true")?.render(context! {})?
    );
    println!(
        "  false -> {}",
        env_default
            .get_template("bool_false")?
            .render(context! {})?
    );
    println!(
        "  none -> {}",
        env_default.get_template("none_val")?.render(context! {})?
    );

    println!("PyCompat mode:");
    println!(
        "  true -> {}",
        env_pycompat
            .get_template("bool_true")?
            .render(context! {})?
    );
    println!(
        "  false -> {}",
        env_pycompat
            .get_template("bool_false")?
            .render(context! {})?
    );
    println!(
        "  none -> {}",
        env_pycompat.get_template("none_val")?.render(context! {})?
    );

    Ok(())
}
