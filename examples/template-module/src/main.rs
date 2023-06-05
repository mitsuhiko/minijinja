use minijinja::value::Value;
use minijinja::{context, Environment};

fn main() {
    let mut env = Environment::new();
    env.add_template("layout.html", include_str!("templates/layout.html"))
        .unwrap();
    env.add_template("index.html", include_str!("templates/index.html"))
        .unwrap();

    let template = env.get_template("index.html").unwrap();
    let mut module = template
        .eval_to_module(context! {
            site_url => "http://example.com",
        })
        .unwrap();

    println!("Module API:");
    println!(
        "  block 'title': {:?}",
        module.render_block("title").unwrap()
    );
    println!("  block 'body': {:?}", module.render_block("body").unwrap());
    println!(
        "  Macro 'utility': {:?}",
        module.call_macro("utility", &[]).unwrap()
    );
    println!(
        "  Variable 'global_variable': {:?}",
        module.get_export("global_variable")
    );
    println!("  Exports: {:?}", module.exports());

    println!();
    println!("State API:");
    let state = module.state();
    println!("  Template name: {:?}", state.name());
    println!("  Undefined behavior: {:?}", state.undefined_behavior());
    println!(
        "  Range function resolved: {:?}",
        state.lookup("range").unwrap()
    );
    println!(
        "  Range function invoked: {:?}",
        state
            .lookup("range")
            .unwrap()
            .call(state, &[Value::from(5)])
            .unwrap()
    );
}
