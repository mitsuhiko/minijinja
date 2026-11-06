//! Templates that try to exhaust memory or time must run out of fuel.
//!
//! The tests use a counting allocator to check that the memory a render
//! allocates stays proportional to the fuel it was given.
#![cfg(all(feature = "fuel", feature = "builtins", feature = "macros"))]
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use minijinja::value::Value;
use minijinja::{context, Environment, ErrorKind};

struct CountingAllocator;

thread_local! {
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

fn note_alloc(size: usize) {
    // `try_with` as the thread local might already be gone during thread
    // shutdown.
    let _ = ALLOCATED.try_with(|x| x.set(x.get().saturating_add(size)));
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_alloc(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_alloc(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note_alloc(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn allocated() -> usize {
    ALLOCATED.with(|x| x.get())
}

/// The fuel given to the templates in this test.
const FUEL: u64 = 100_000;

/// The engine charges one fuel per 64 bytes.  Operations that cannot check
/// sizes ahead of time may overshoot by the size of their result, so allow a
/// generous factor on top.
const MAX_BYTES: usize = FUEL as usize * 64 * 4;

/// An iterator that never ends.
fn infinite() -> Value {
    Value::make_iterable(|| (0u64..).map(Value::from))
}

/// A long but finite iterator that does not know its length.
fn long_iter() -> Value {
    Value::make_iterable(|| (0u64..100_000_000).map(Value::from))
}

fn render_with_fuel(source: &str, fuel: u64) -> (Result<String, minijinja::Error>, usize) {
    let mut env = Environment::new();
    env.set_fuel(Some(fuel));
    let ctx = context! {
        infinite => infinite(),
        long_iter => long_iter(),
        big => "x".repeat(1_000_000),
        big_list => (0..100_000).collect::<Vec<_>>(),
        big_map => (0..100_000).map(|x| (x.to_string(), x)).collect::<std::collections::BTreeMap<_, _>>(),
    };
    let before = allocated();
    let rv = env.render_str(source, ctx);
    (rv, allocated() - before)
}

fn root_cause(err: &minijinja::Error) -> ErrorKind {
    let mut err: &dyn std::error::Error = err;
    let mut kind = ErrorKind::InvalidOperation;
    loop {
        if let Some(mj) = err.downcast_ref::<minijinja::Error>() {
            kind = mj.kind();
        }
        match err.source() {
            Some(source) => err = source,
            None => return kind,
        }
    }
}

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
    // filters with size arguments.  The sizes fit into a 32-bit usize, as
    // larger ones are rejected as out of range there before fuel is checked.
    "{{ [1]|slice(1000000000) }}",
    "{{ [1]|batch(1000000000, 'x') }}",
    "{{ 'a\\nb'|indent(1000000000) }}",
    "{{ '%.1000000000f'|format(1.0) }}",
    "{{ '%1000000000d'|format(1) }}",
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

#[test]
fn test_bombs_run_out_of_fuel() {
    let mut failures = Vec::new();
    for source in BOMBS {
        let start = std::time::Instant::now();
        let (rv, bytes) = render_with_fuel(source, FUEL);
        let elapsed = start.elapsed();
        let kind = rv.as_ref().err().map(root_cause);
        println!(
            "{:>12} bytes {:>8.1}ms {:?} {}",
            bytes,
            elapsed.as_secs_f64() * 1000.0,
            kind,
            source
        );
        if kind != Some(ErrorKind::OutOfFuel) {
            failures.push(format!("did not run out of fuel: {source} ({kind:?})"));
        }
        if bytes > MAX_BYTES {
            failures.push(format!("allocated {bytes} bytes: {source}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
