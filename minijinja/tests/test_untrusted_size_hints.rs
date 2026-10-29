//! Iterators provided by objects can report arbitrary size hints.  The engine
//! must not trust them for preallocations as that can abort the process.
use minijinja::value::Value;
use minijinja::{context, Environment};

/// An iterator that yields three items but lies about its size.
struct LyingIter {
    pos: usize,
    exact: bool,
}

impl Iterator for LyingIter {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        if self.pos < 3 {
            self.pos += 1;
            Some(Value::from(self.pos))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.exact {
            (usize::MAX, Some(usize::MAX))
        } else {
            (usize::MAX, None)
        }
    }
}

fn lying_iterable(exact: bool) -> Value {
    Value::make_iterable(move || LyingIter { pos: 0, exact })
}

fn render(source: &str, items: Value) -> String {
    let env = Environment::new();
    env.render_str(source, context!(items)).unwrap()
}

#[test]
fn test_untrusted_iterator_size_hints() {
    for exact in [false, true] {
        for (source, expected) in [
            ("{{ items|list }}", "[1, 2, 3]"),
            ("{{ items|sort(reverse=true) }}", "[3, 2, 1]"),
            ("{{ items|reverse|list }}", "[3, 2, 1]"),
            ("{{ items|unique|list }}", "[1, 2, 3]"),
            ("{{ items|join(',') }}", "1,2,3"),
            ("{{ items|slice(2) }}", "[[1, 2], [3]]"),
            ("{{ items|batch(2) }}", "[[1, 2], [3]]"),
            ("{{ items|map('string')|list }}", "['1', '2', '3']"),
            ("{{ items[::-1]|list }}", "[3, 2, 1]"),
            ("{{ items[:2]|list }}", "[1, 2]"),
        ] {
            assert_eq!(
                render(source, lying_iterable(exact)),
                expected,
                "{source} (exact={exact})"
            );
        }
    }
}

#[test]
fn test_huge_batch_count() {
    // the count must fit into a usize on the target (32bit on wasm32)
    let env = Environment::new();
    let rv = env
        .render_str(
            "{{ items|batch(count) }}",
            context!(items => vec![1, 2], count => usize::MAX / 4),
        )
        .unwrap();
    assert_eq!(rv, "[[1, 2]]");
}
