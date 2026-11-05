//! Measures how fuel relates to time and memory.
//!
//! Every workload is rendered with unlimited fuel and the fuel it consumed is
//! compared to the time it took and the memory it allocated.  For fuel to be
//! a good budget, nanoseconds per fuel and bytes per fuel should be roughly
//! the same for every workload.
//!
//! Run with:
//!
//! ```text
//! $ cargo run --release -p benchmarks --features fuel --bin fuel_calibration
//! ```
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use minijinja::value::Value;
use minijinja::{context, Environment, State};

struct CountingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn note_alloc(size: usize) {
    ALLOCATED.fetch_add(size, Ordering::Relaxed);
    let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_alloc(layout.size());
        System.alloc(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        note_alloc(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

const ALL_ELEMENTS: &str = include_str!("../../inputs/all_elements.html");
const STRING_HEAVY: &str = include_str!("../../inputs/string_heavy.html");
const MACRO_HEAVY: &str = include_str!("../../inputs/macro_heavy.html");
const FOOTER: &str = include_str!("../../inputs/footer.html");

struct Workload {
    group: &'static str,
    name: &'static str,
    template: &'static str,
    ctx: fn() -> Value,
}

fn no_ctx() -> Value {
    context! {}
}

fn data_ctx() -> Value {
    context! {
        items => (0..1000).collect::<Vec<_>>(),
        words => (0..1000).map(|x| format!("word{x}")).collect::<Vec<_>>(),
        users => (0..1000).map(|x| context! {
            name => format!("user{x}"),
            age => x % 90,
            city => format!("city{}", x % 17),
        }).collect::<Vec<_>>(),
        mapping => (0..1000).map(|x| (format!("key{x}"), x)).collect::<std::collections::BTreeMap<_, _>>(),
        text => "lorem ipsum dolor sit amet ".repeat(400),
        lines => "lorem ipsum dolor sit amet\n".repeat(400),
        iter => Value::make_iterable(|| (0..1000).map(Value::from)),
    }
}

fn all_elements_ctx() -> Value {
    context! {
        DEBUG => false,
        site => context! {
            nav => vec![
                context!{url => "/", is_active => true, title => "Index"},
                context!{url => "/doc", is_active => false, title => "Docs"},
                context!{url => "/help", is_active => false, title => "Help"},
            ]
        },
        items => (0..200).skip(3).collect::<Vec<_>>(),
    }
}

fn string_heavy_ctx() -> Value {
    context! {
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
                recent_posts => (0..8).map(|x| context!{
                    url => format!("/posts/{x}"),
                    title => "Understanding Template Engine Internals & Performance Characteristics",
                    date => "2026-03-10",
                    featured => x % 3 == 0,
                }).collect::<Vec<_>>(),
            },
        },
    }
}

fn macro_heavy_ctx() -> Value {
    context! {
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
        items => (0..4).map(|x| context!{
            title => format!("Project {x}"),
            description => "A powerful Jinja2-compatible template engine for Rust with minimal dependencies and excellent performance characteristics.",
            featured => x % 2 == 0,
            metadata => context!{ language => "Rust", license => "Apache-2.0", stars => "2.1k" },
            actions => vec![
                context!{ url => "/projects/minijinja", label => "View", style => "primary" },
                context!{ url => "/projects/minijinja/edit", label => "Edit", style => "secondary" },
            ],
        }).collect::<Vec<_>>(),
        notifications => vec![
            context!{ level => "info", title => "Update Available", message => "Version 3.0 is ready to install.", action => context!{ url => "/update", label => "Install Now" } },
            context!{ level => "warning", title => "API Quota", message => "You have used 85% of your monthly API quota." },
        ],
    }
}

const WORKLOADS: &[Workload] = &[
    // realistic templates
    Workload { group: "real", name: "all_elements", template: "{% include 'all_elements.html' %}", ctx: all_elements_ctx },
    Workload { group: "real", name: "string_heavy", template: "{% include 'string_heavy.html' %}", ctx: string_heavy_ctx },
    Workload { group: "real", name: "macro_heavy", template: "{% include 'macro_heavy.html' %}", ctx: macro_heavy_ctx },
    Workload { group: "real", name: "loop_attrs", template: "{% for u in users %}<li>{{ u.name }} ({{ u.age }}) from {{ u.city|title }}</li>{% endfor %}", ctx: data_ctx },
    Workload { group: "real", name: "loop_if", template: "{% for x in items %}{% if x is even %}{{ x }}{% else %}-{% endif %}{% endfor %}", ctx: data_ctx },
    // instructions
    Workload { group: "instr", name: "empty_loop", template: "{% for x in range(10000) %}{% endfor %}", ctx: no_ctx },
    Workload { group: "instr", name: "arith", template: "{% for x in range(10000) %}{% set y = x * 2 + 1 %}{% endfor %}", ctx: no_ctx },
    Workload { group: "instr", name: "emit_int", template: "{% for x in range(10000) %}{{ x }}{% endfor %}", ctx: no_ctx },
    // output
    Workload { group: "output", name: "emit_text", template: "{% for x in range(1000) %}{{ text }}{% endfor %}", ctx: data_ctx },
    Workload { group: "output", name: "raw_text", template: "{% for x in range(10000) %}lorem ipsum dolor sit amet lorem ipsum dolor sit amet lorem ipsum dolor sit amet lorem ipsum dolor sit amet{% endfor %}", ctx: no_ctx },
    Workload { group: "output", name: "capture", template: "{% for x in range(1000) %}{% set y %}{{ text }}{% endset %}{% endfor %}", ctx: data_ctx },
    Workload { group: "output", name: "escape", template: "{% autoescape true %}{% for x in range(1000) %}{{ text }}{% endfor %}{% endautoescape %}", ctx: data_ctx },
    // string building
    Workload { group: "string", name: "repeat", template: "{% for x in range(1000) %}{% set y = 'abc' * 1000 %}{% endfor %}", ctx: no_ctx },
    Workload { group: "string", name: "concat", template: "{% for x in range(1000) %}{% set y = text ~ text %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "grow", template: "{% set ns = namespace(s='') %}{% for x in range(3000) %}{% set ns.s = ns.s ~ 'abcdefgh' %}{% endfor %}", ctx: no_ctx },
    Workload { group: "string", name: "upper", template: "{% for x in range(1000) %}{% set y = text|upper %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "replace", template: "{% for x in range(1000) %}{% set y = text|replace('o', '00') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "length", template: "{% for x in range(1000) %}{% set y = text|length %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "split", template: "{% for x in range(1000) %}{% set y = text|split %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "lines", template: "{% for x in range(1000) %}{% set y = lines|lines %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "indent", template: "{% for x in range(1000) %}{% set y = lines|indent(8) %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "format", template: "{% for x in range(10000) %}{% set y = '%10s|%-8d|%.3f'|format('a', x, 1.5) %}{% endfor %}", ctx: no_ctx },
    Workload { group: "string", name: "in_text", template: "{% for x in range(1000) %}{% set y = 'xyz' in text %}{% endfor %}", ctx: data_ctx },
    Workload { group: "string", name: "slice", template: "{% for x in range(1000) %}{% set y = text[10:-10] %}{% endfor %}", ctx: data_ctx },
    // sequences
    Workload { group: "seq", name: "list", template: "{% for x in range(100) %}{% set y = items|list %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "list_iter", template: "{% for x in range(100) %}{% set y = iter|list %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "sort", template: "{% for x in range(100) %}{% set y = words|sort %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "sort_attr", template: "{% for x in range(100) %}{% set y = users|sort(attribute='age') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "join", template: "{% for x in range(100) %}{% set y = words|join(', ') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "join_ints", template: "{% for x in range(100) %}{% set y = items|join(', ') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "unique", template: "{% for x in range(100) %}{% set y = words|unique %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "sum", template: "{% for x in range(100) %}{% set y = items|sum %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "max", template: "{% for x in range(100) %}{% set y = items|max %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "reverse", template: "{% for x in range(100) %}{% set y = iter|reverse %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "last", template: "{% for x in range(100) %}{% set y = iter|last %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "map_attr", template: "{% for x in range(100) %}{% set y = users|map(attribute='name') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "map_filter", template: "{% for x in range(100) %}{% set y = words|map('upper') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "select", template: "{% for x in range(100) %}{% set y = items|select('even') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "selectattr", template: "{% for x in range(100) %}{% set y = users|selectattr('age', 'even') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "batch", template: "{% for x in range(100) %}{% set y = items|batch(3) %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "slice", template: "{% for x in range(100) %}{% set y = items|slice(3) %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "groupby", template: "{% for x in range(100) %}{% set y = users|groupby('city') %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "dictsort", template: "{% for x in range(100) %}{% set y = mapping|dictsort %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "in_list", template: "{% for x in range(100) %}{% set y = -1 in items %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "in_iter", template: "{% for x in range(100) %}{% set y = -1 in iter %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "eq", template: "{% for x in range(100) %}{% set y = items == items|list %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "tojson", template: "{% for x in range(100) %}{% set y = users|tojson %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "emit_list", template: "{% for x in range(100) %}{{ items }}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "emit_users", template: "{% for x in range(100) %}{{ users }}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "pprint", template: "{% for x in range(100) %}{% set y = users|pprint %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "string", template: "{% for x in range(100) %}{% set y = items|string %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "literal", template: "{% for x in range(10000) %}{% set y = [x, x, x, x] %}{% endfor %}", ctx: no_ctx },
    Workload { group: "seq", name: "map_literal", template: "{% for x in range(10000) %}{% set y = {'a': x, 'b': x} %}{% endfor %}", ctx: no_ctx },
    Workload { group: "seq", name: "add", template: "{% for x in range(100) %}{% set y = (items + items)|list %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "rev_slice", template: "{% for x in range(100) %}{% set y = iter[::-1] %}{% endfor %}", ctx: data_ctx },
    Workload { group: "seq", name: "index_iter", template: "{% for x in range(100) %}{% set y = iter[999] %}{% endfor %}", ctx: data_ctx },
];

const BOMBS: &[&str] = &[
    // output
    "{% for i in range(100000) %}{{ big }}{% endfor %}",
    "{% for i in range(100000) %}{% set x %}{{ big }}{% endset %}{% endfor %}",
    "{% macro m() %}{{ big }}{% endmacro %}{% for i in range(100000) %}{% set x = m() %}{% endfor %}",
    // repetition and concatenation
    "{{ 'x' * 100000000 }}",
    "{{ ('x' * 100000000)|list }}",
    "{% for i in range(100000) %}{% set s = 'x' * 1000000 %}{% endfor %}",
    "{% set ns = namespace(s='x') %}{% for i in range(60) %}{% set ns.s = ns.s ~ ns.s %}{% endfor %}",
    "{% set ns = namespace(s='x') %}{% for i in range(60) %}{% set ns.s = ns.s + ns.s %}{% endfor %}",
    "{% set ns = namespace(s=[1]) %}{% for i in range(60) %}{% set ns.s = (ns.s + ns.s)|list %}{% endfor %}",
    "{% for i in range(100000) %}{% set x = big ~ big %}{% endfor %}",
    "{{ ((1,) * 100000000)|length }}",
    "{{ ([1] * 100000000)|list|length }}",
    // host iterables
    "{{ infinite|list }}",
    "{{ infinite|sort }}",
    "{{ infinite|join(',') }}",
    "{{ infinite|reverse }}",
    "{{ infinite|last }}",
    "{{ infinite|unique }}",
    "{{ infinite|sum }}",
    "{{ infinite|max }}",
    "{{ infinite|map('string')|list }}",
    "{{ infinite|select('odd')|list }}",
    "{{ infinite|batch(3)|list }}",
    "{{ infinite|slice(3)|list }}",
    "{{ infinite|groupby('x') }}",
    "{{ -1 in infinite }}",
    "{{ infinite[1000000000000] }}",
    "{% for x in infinite %}{% endfor %}",
    "{% for a, b in infinite|batch(2) %}{% endfor %}",
    "{{ long_iter|list|length }}",
    "{{ long_iter|reverse|first }}",
    "{{ long_iter[::-1]|first }}",
    // filters with size arguments
    "{{ [1]|slice(1000000000000) }}",
    "{{ [1]|batch(1000000000000, 'x') }}",
    "{{ 'a\\nb'|indent(1000000000000) }}",
    "{{ '%.1000000000000f'|format(1.0) }}",
    "{{ '%1000000000000d'|format(1) }}",
    "{{ ('a' * 10000)|replace('a', 'b' * 10000) }}",
    "{{ ([1] * 1000000)|join('x' * 1000) }}",
    "{{ ('x' * 100000)|join('y' * 1000) }}",
    // work on big data
    "{% for i in range(100000) %}{{ big_list|sort|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|last }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|join|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big|upper|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ -1 in big_list }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list == big_list|list }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_map|dictsort|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|unique|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|groupby('x')|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|map('string')|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|select('odd')|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list[::-1]|first }}{% endfor %}",
    "{% for i in range(100000) %}{{ (big ~ ' ')|split|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|tojson|length }}{% endfor %}",
    "{% for i in range(100000) %}{{ big_list|string|length }}{% endfor %}",
];

fn bomb_ctx() -> Value {
    context! {
        infinite => Value::make_iterable(|| (0u64..).map(Value::from)),
        long_iter => Value::make_iterable(|| (0u64..100_000_000).map(Value::from)),
        big => "x".repeat(1_000_000),
        big_list => (0..100_000).collect::<Vec<_>>(),
        big_map => (0..100_000).map(|x| (x.to_string(), x)).collect::<std::collections::BTreeMap<_, _>>(),
    }
}

/// Runs the templates from the fuel limit tests with different budgets and
/// reports the worst time and memory.
fn run_bombs() {
    println!(
        "{:>10} {:>12} {:>12} {:>14} {:>14}  worst template",
        "fuel", "max time", "time/fuel", "max peak", "peak/fuel"
    );
    for fuel in [10_000u64, 100_000, 1_000_000, 10_000_000] {
        let mut env = Environment::new();
        env.set_fuel(Some(fuel));
        let ctx = bomb_ctx();
        let mut worst_time = (0.0, "");
        let mut worst_peak = (0, "");
        for source in BOMBS {
            let live_before = LIVE.load(Ordering::Relaxed);
            PEAK.store(live_before, Ordering::Relaxed);
            let start = Instant::now();
            // with large budgets some of these legitimately succeed
            let rv = env.render_str(source, ctx.clone());
            let elapsed = start.elapsed().as_secs_f64();
            drop(rv);
            let peak = PEAK.load(Ordering::Relaxed) - live_before;
            if elapsed > worst_time.0 {
                worst_time = (elapsed, source);
            }
            if peak > worst_peak.0 {
                worst_peak = (peak, source);
            }
        }
        println!(
            "{:>10} {:>10.1}ms {:>10.1}ns {:>12.1}MB {:>14.1}  {} / {}",
            fuel,
            worst_time.0 * 1000.0,
            worst_time.0 * 1e9 / fuel as f64,
            worst_peak.0 as f64 / 1e6,
            worst_peak.0 as f64 / fuel as f64,
            worst_time.1,
            worst_peak.1,
        );
    }
}

fn make_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("footer.html", FOOTER).unwrap();
    env.add_template("all_elements.html", ALL_ELEMENTS).unwrap();
    env.add_template("string_heavy.html", STRING_HEAVY).unwrap();
    env.add_template("macro_heavy.html", MACRO_HEAVY).unwrap();
    env.add_filter("asset_url", |_: &State, value: String| Ok(value));
    env.add_function("current_year", |_: &State| Ok(2022));
    env.set_fuel(Some(u64::MAX / 1024));
    env
}

struct Measurement {
    fuel: u64,
    ns: f64,
    allocated: usize,
    peak: usize,
}

fn measure(env: &Environment, workload: &Workload) -> Measurement {
    let tmpl = env.template_from_str(workload.template).unwrap();
    let ctx = (workload.ctx)();

    // warm up and find the fuel
    let mut fuel = 0;
    for _ in 0..3 {
        let rv = tmpl.render_captured(ctx.clone()).unwrap();
        fuel = rv.state().fuel_levels().unwrap().0;
    }

    // memory
    let before = ALLOCATED.load(Ordering::Relaxed);
    let live_before = LIVE.load(Ordering::Relaxed);
    PEAK.store(live_before, Ordering::Relaxed);
    let rv = tmpl.render_captured(ctx.clone()).unwrap();
    let allocated = ALLOCATED.load(Ordering::Relaxed) - before;
    let peak = PEAK.load(Ordering::Relaxed) - live_before;
    drop(rv);

    // time.  The fastest sample is used as it's the least affected by other
    // load on the machine.
    let mut samples = Vec::new();
    for _ in 0..25 {
        let start = Instant::now();
        let mut n = 0;
        while start.elapsed().as_millis() < 20 || n < 3 {
            std::hint::black_box(tmpl.render_captured(ctx.clone()).unwrap());
            n += 1;
        }
        samples.push(start.elapsed().as_nanos() as f64 / n as f64);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());

    Measurement {
        fuel,
        ns: samples[0],
        allocated,
        peak,
    }
}

fn main() {
    let filter = std::env::args().nth(1);
    if filter.as_deref() == Some("--bombs") {
        run_bombs();
        return;
    }
    let env = make_env();
    // the instruction workloads are the baseline that everything else is
    // compared to.
    let baseline = {
        let mut rates: Vec<f64> = WORKLOADS
            .iter()
            .filter(|w| w.group == "instr")
            .map(|w| {
                let m = measure(&env, w);
                m.ns / m.fuel.max(1) as f64
            })
            .collect();
        rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        rates[rates.len() / 2]
    };
    println!("baseline: {baseline:.2} ns/fuel");
    println!(
        "{:<8} {:<14} {:>10} {:>12} {:>9} {:>7} {:>10} {:>10}",
        "group", "workload", "fuel", "time", "ns/fuel", "rel", "alloc/fuel", "peak/fuel"
    );
    for workload in WORKLOADS {
        if let Some(ref filter) = filter {
            if !workload.name.contains(filter.as_str()) && workload.group != filter {
                continue;
            }
        }
        let m = measure(&env, workload);
        let fuel = m.fuel.max(1) as f64;
        println!(
            "{:<8} {:<14} {:>10} {:>10.1}µs {:>9.2} {:>7.2} {:>10.1} {:>10.1}",
            workload.group,
            workload.name,
            m.fuel,
            m.ns / 1000.0,
            m.ns / fuel,
            m.ns / fuel / baseline,
            m.allocated as f64 / fuel,
            m.peak as f64 / fuel,
        );
    }
}
