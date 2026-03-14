use std::time::Instant;

use minijinja::machinery::parse;
use minijinja::{context, Environment, State};

const ALL_ELEMENTS: &str = include_str!("../../inputs/all_elements.html");

fn do_parse() {
    parse(
        std::hint::black_box(ALL_ELEMENTS),
        "all_elements.html",
        Default::default(),
        Default::default(),
    )
    .unwrap();
}

fn do_compile() {
    let mut env = Environment::new();
    env.add_template("all_elements.html", ALL_ELEMENTS).unwrap();
}

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
    env.add_template("all_elements.html", ALL_ELEMENTS).unwrap();
    env.add_filter("asset_url", |_: &State, value: String| Ok(value));
    env.add_function("current_year", |_: &State| Ok(2022));
    env
}

fn bench_median_ns(mut f: impl FnMut(), rounds: usize, iters_per_round: usize) -> f64 {
    let mut samples = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let start = Instant::now();
        for _ in 0..iters_per_round {
            f();
        }
        samples.push(start.elapsed().as_nanos() as f64 / iters_per_round as f64);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn main() {
    let env = create_real_env();

    // Warmup to avoid startup noise.
    for _ in 0..100 {
        do_parse();
        do_compile();
        std::hint::black_box(do_render(&env));
    }

    let parse_ns = bench_median_ns(do_parse, 15, 256);
    let compile_ns = bench_median_ns(do_compile, 15, 128);

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
    println!("METRIC parse_ns={parse_ns:.2}");
    println!("METRIC compile_ns={compile_ns:.2}");
}
