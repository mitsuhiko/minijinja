use std::hint::black_box;
use std::time::Instant;

use minijinja::{context, Environment};
use serde::Serialize;

const INDEX_SOURCE: &str = include_str!("../../inputs/comparison/minijinja.html");
const FOOTER_SOURCE: &str = include_str!("../../inputs/comparison/minijinja_footer.html");

#[derive(Serialize, Debug)]
struct NavItem {
    url: &'static str,
    title: &'static str,
    is_active: bool,
}

#[derive(Serialize, Debug)]
struct Site {
    nav: Vec<NavItem>,
    copyright: u32,
}

#[derive(Serialize, Debug)]
struct Context {
    items: Vec<String>,
    site: Site,
    title: &'static str,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            items: vec![
                "<First Item>".into(),
                "<Second Item>".into(),
                "<Third Item>".into(),
                "<Fourth Item>".into(),
                "<Fifth Item>".into(),
                "<Sixth Item>".into(),
            ],
            site: Site {
                nav: vec![
                    NavItem {
                        url: "/",
                        title: "Index",
                        is_active: true,
                    },
                    NavItem {
                        url: "/download",
                        title: "Download",
                        is_active: false,
                    },
                    NavItem {
                        url: "/about",
                        title: "About",
                        is_active: false,
                    },
                    NavItem {
                        url: "/help",
                        title: "Help",
                        is_active: false,
                    },
                ],
                copyright: 2022,
            },
            title: "My Benchmark Site",
        }
    }
}

fn do_compile(env: &mut Environment<'_>) {
    env.add_template("template.html", black_box(INDEX_SOURCE))
        .unwrap();
    env.add_template("footer.html", black_box(FOOTER_SOURCE))
        .unwrap();
}

fn create_render_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("template.html", INDEX_SOURCE).unwrap();
    env.add_template("footer.html", FOOTER_SOURCE).unwrap();
    env
}

fn do_render(env: &Environment<'_>) -> String {
    env.get_template("template.html")
        .unwrap()
        .render(black_box(Context::default()))
        .unwrap()
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
    let mut compile_env = Environment::new();
    let render_env = create_render_env();

    for _ in 0..100 {
        do_compile(&mut compile_env);
        black_box(do_render(&render_env));
    }

    let compile_ns = bench_median_ns(|| do_compile(&mut compile_env), 21, 512);
    let render_ns = bench_median_ns(
        || {
            black_box(do_render(&render_env));
        },
        41,
        128,
    );

    let total_ns = compile_ns + render_ns;

    println!("METRIC comparison_ns={total_ns:.2}");
    println!("METRIC comparison_compile_ns={compile_ns:.2}");
    println!("METRIC comparison_render_ns={render_ns:.2}");

    // keep one structured context materialization in the binary so macro-generated
    // code remains linked similarly to regular benchmark paths.
    black_box(context! {
        title => "My Benchmark Site",
    });
}
