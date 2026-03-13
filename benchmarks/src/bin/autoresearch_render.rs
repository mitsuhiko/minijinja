use std::time::Instant;

use minijinja::{context, Environment, State};

fn do_render(env: &Environment) -> String {
    let tmpl = env.get_template("all_elements.html").unwrap();
    tmpl.render(context! {
        DEBUG => false,
        site => context! {
            nav => vec![
                context!{url => "/", is_active => true, title => "Index"},
                context!{url => "/doc", is_active => false, title => "Docs"},
                context!{url => "/help", is_active => false, title => "Help"},
            ]
        },
        items => (0..200).skip(3).collect::<Vec<_>>(),
    })
    .unwrap()
}

fn create_real_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("footer.html", include_str!("../../inputs/footer.html"))
        .unwrap();
    env.add_template(
        "all_elements.html",
        include_str!("../../inputs/all_elements.html"),
    )
    .unwrap();
    env.add_filter("asset_url", |_: &State, value: String| Ok(value));
    env.add_function("current_year", |_: &State| Ok(2022));
    env
}

fn main() {
    let env = create_real_env();

    // Warmup to avoid startup noise.
    for _ in 0..200 {
        std::hint::black_box(do_render(&env));
    }

    let rounds = 41;
    let iters_per_round = 128;
    let mut samples = Vec::with_capacity(rounds);

    for _ in 0..rounds {
        let start = Instant::now();
        for _ in 0..iters_per_round {
            std::hint::black_box(do_render(&env));
        }
        let ns_per_render = start.elapsed().as_nanos() as f64 / iters_per_round as f64;
        samples.push(ns_per_render);
    }

    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median_ns = samples[samples.len() / 2];
    let mean_ns = samples.iter().sum::<f64>() / samples.len() as f64;

    println!("METRIC render_ns={median_ns:.2}");
    println!("METRIC render_mean_ns={mean_ns:.2}");
}
