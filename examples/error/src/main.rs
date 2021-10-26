use minijinja::{context, Environment, Error};

fn render_error_context(err: &Error, env: &Environment<'_>) -> String {
    use std::fmt::Write;

    let tmpl = err.name().and_then(|x| env.get_template(x).ok());
    let mut rv = String::new();

    writeln!(rv, "{:-^1$}", " Source Context ", 74).unwrap();
    if let Some(tmpl) = tmpl {
        let lines: Vec<_> = tmpl.source().lines().enumerate().collect();
        let idx = err.line().unwrap_or(1) - 1;
        let skip = idx.saturating_sub(3);
        let pre = lines.iter().skip(skip).take(3.min(idx)).collect::<Vec<_>>();
        let post = lines.iter().skip(idx + 1).take(3).collect::<Vec<_>>();
        for (idx, line) in pre {
            writeln!(rv, "{:>4} | {}", idx + 1, line).unwrap();
        }
        writeln!(rv, "{:>4} > {}", idx + 1, lines[idx].1).unwrap();
        for (idx, line) in post {
            writeln!(rv, "{:>4} | {}", idx + 1, line).unwrap();
        }
    } else {
        writeln!(rv, "source not available").unwrap();
    }
    write!(rv, "{:-^1$}", "", 74).unwrap();

    rv
}

fn main() {
    let mut env = Environment::new();
    env.add_template(
        "hello.txt",
        r#"
        first line
        {% for item in seq %}
            Hello {{ item + bar }}!
        {% endfor %}
        last line
        "#,
    )
    .unwrap();
    let template = env.get_template("hello.txt").unwrap();
    let ctx = context! {
        seq => vec![1, 2, 3],
        bar => "test"
    };
    match template.render(&ctx) {
        Ok(result) => println!("{}", result),
        Err(err) => {
            eprintln!("Template Failed Rendering:");
            eprintln!("  {}", err);
            eprintln!("{}", render_error_context(&err, &env));
            eprintln!("Render Context: {:#?}", ctx);
        }
    }
}
