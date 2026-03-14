use std::time::Instant;

use minijinja::machinery::parse;
use minijinja::{context, Environment, State};

const ALL_ELEMENTS: &str = include_str!("../../inputs/all_elements.html");
const STRING_HEAVY: &str = include_str!("../../inputs/string_heavy.html");
const MACRO_HEAVY: &str = include_str!("../../inputs/macro_heavy.html");

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

// --- Benchmark 1: all_elements (loop-heavy, integer upper, includes, blocks) ---

fn create_all_elements_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("footer.html", include_str!("../../inputs/footer.html"))
        .unwrap();
    env.add_template("all_elements.html", ALL_ELEMENTS).unwrap();
    env.add_filter("asset_url", |_: &State, value: String| Ok(value));
    env.add_function("current_year", |_: &State| Ok(2022));
    env
}

fn do_render_all_elements(env: &Environment) -> String {
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

// --- Benchmark 2: string_heavy (filters, escaping, nested objects, varied strings) ---

fn create_string_heavy_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("string_heavy.html", STRING_HEAVY).unwrap();
    env
}

fn do_render_string_heavy(env: &Environment) -> String {
    let tmpl = env.get_template("string_heavy.html").unwrap();
    tmpl.render(context! {
        post => context! {
            title => "the quick brown fox jumps over the lazy dog",
            date => "2026-03-14",
            tags => vec!["Rust Programming", "Template Engines", "Performance", "Web Development", "Open Source"],
            summary => "This is a longer summary that contains various characters like <html> tags, \"quotes\", and other things that need escaping. It should be long enough to exercise the truncate filter and HTML escaping paths in a meaningful way for benchmarking purposes.",
            paragraphs => vec![
                "MiniJinja is a powerful template engine for Rust that implements a large subset of the Jinja2 template language.",
                "It focuses on providing a minimal dependency footprint while still supporting advanced features like template inheritance, macros, and custom filters.",
                "Performance is a key goal, with careful attention paid to minimizing allocations and optimizing hot paths in the rendering pipeline.",
                "The engine supports auto-escaping, making it safe to use with HTML output without worrying about XSS vulnerabilities.",
                "Custom filters and functions can be registered to extend the engine's capabilities for specific use cases.",
            ],
            footer_note => "Originally published on the <em>Rust Blog</em> &mdash; all rights reserved.",
            author => context! {
                name => "Armin Ronacher",
                recent_posts => vec![
                    context!{ url => "/posts/1", title => "Understanding Template Engine Internals & Performance Characteristics", date => "2026-03-10", featured => true },
                    context!{ url => "/posts/2", title => "Building Safe Abstractions in Rust", date => "2026-03-05", featured => false },
                    context!{ url => "/posts/3", title => "Zero-Copy Parsing Techniques for High-Performance Applications", date => "2026-02-28", featured => false },
                    context!{ url => "/posts/4", title => "The Art of Minimizing Dependencies in Library Design", date => "2026-02-20", featured => true },
                    context!{ url => "/posts/5", title => "Advanced Error Handling Patterns & Best Practices", date => "2026-02-15", featured => false },
                    context!{ url => "/posts/6", title => "Comparing Jinja2 Implementations Across Languages", date => "2026-02-10", featured => false },
                    context!{ url => "/posts/7", title => "Memory-Efficient String Representations", date => "2026-02-05", featured => true },
                    context!{ url => "/posts/8", title => "Bytecode Compilation for Template Languages", date => "2026-01-30", featured => false },
                ],
            },
        },
    })
    .unwrap()
}

// --- Benchmark 3: macro_heavy (macros, conditionals, metadata iteration, varied types) ---

fn create_macro_heavy_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("macro_heavy.html", MACRO_HEAVY).unwrap();
    env
}

fn do_render_macro_heavy(env: &Environment) -> String {
    let tmpl = env.get_template("macro_heavy.html").unwrap();
    tmpl.render(context! {
        page => context! {
            title => "Dashboard Settings",
            form_action => "/api/settings/save",
            items_title => "Your Projects",
        },
        user => context! {
            first_name => "Jane",
            last_name => "Doe",
            email => "jane.doe@example.com",
            bio => "Software engineer focused on systems programming and developer tools.",
            role_options => vec![
                context!{ value => "admin", label => "Administrator", selected => false },
                context!{ value => "editor", label => "Editor", selected => true },
                context!{ value => "viewer", label => "Viewer", selected => false },
            ],
        },
        items => vec![
            context!{
                title => "MiniJinja",
                description => "A powerful Jinja2-compatible template engine for Rust with minimal dependencies and excellent performance characteristics.",
                featured => true,
                metadata => context!{ language => "Rust", license => "Apache-2.0", stars => "2.1k" },
                actions => vec![
                    context!{ url => "/projects/minijinja", label => "View", style => "primary" },
                    context!{ url => "/projects/minijinja/edit", label => "Edit", style => "secondary" },
                ],
            },
            context!{
                title => "Insta",
                description => "A snapshot testing library for Rust that makes it easy to test complex output values by comparing against reference files.",
                featured => false,
                metadata => context!{ language => "Rust", license => "Apache-2.0", stars => "1.8k" },
                actions => vec![
                    context!{ url => "/projects/insta", label => "View", style => "primary" },
                ],
            },
            context!{
                title => "Rye",
                description => "An experimental Python project and package management tool designed to be fast and comprehensive.",
                featured => true,
                metadata => context!{ language => "Rust/Python", license => "MIT", stars => "12k" },
                actions => vec![
                    context!{ url => "/projects/rye", label => "View", style => "primary" },
                    context!{ url => "/projects/rye/settings", label => "Settings", style => "default" },
                ],
            },
            context!{
                title => "Similar",
                description => "A Rust library implementing different string similarity and diffing algorithms for text comparison.",
                featured => false,
                metadata => context!{ language => "Rust", license => "Apache-2.0", stars => "800" },
                actions => vec![
                    context!{ url => "/projects/similar", label => "View", style => "primary" },
                ],
            },
        ],
        notifications => vec![
            context!{ level => "info", title => "Update Available", message => "Version 3.0 is ready to install.", action => context!{ url => "/update", label => "Install Now" } },
            context!{ level => "warning", title => "API Quota", message => "You have used 85% of your monthly API quota." },
            context!{ level => "success", title => "Deploy Complete", message => "Production deployment finished successfully.", action => context!{ url => "/deploys/latest", label => "View Logs" } },
        ],
    })
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
    let env1 = create_all_elements_env();
    let env2 = create_string_heavy_env();
    let env3 = create_macro_heavy_env();

    // Warmup
    for _ in 0..100 {
        do_parse();
        do_compile();
        std::hint::black_box(do_render_all_elements(&env1));
        std::hint::black_box(do_render_string_heavy(&env2));
        std::hint::black_box(do_render_macro_heavy(&env3));
    }

    let parse_ns = bench_median_ns(do_parse, 15, 256);
    let compile_ns = bench_median_ns(do_compile, 15, 128);

    let rounds = 41;
    let iters = 128;

    let render_all_elements_ns = bench_median_ns(
        || {
            std::hint::black_box(do_render_all_elements(&env1));
        },
        rounds,
        iters,
    );
    let render_string_heavy_ns = bench_median_ns(
        || {
            std::hint::black_box(do_render_string_heavy(&env2));
        },
        rounds,
        iters,
    );
    let render_macro_heavy_ns = bench_median_ns(
        || {
            std::hint::black_box(do_render_macro_heavy(&env3));
        },
        rounds,
        iters,
    );

    let render_total_ns = render_all_elements_ns + render_string_heavy_ns + render_macro_heavy_ns;

    println!("METRIC render_ns={render_total_ns:.2}");
    println!("METRIC render_all_elements_ns={render_all_elements_ns:.2}");
    println!("METRIC render_string_heavy_ns={render_string_heavy_ns:.2}");
    println!("METRIC render_macro_heavy_ns={render_macro_heavy_ns:.2}");
    println!("METRIC parse_ns={parse_ns:.2}");
    println!("METRIC compile_ns={compile_ns:.2}");
}
