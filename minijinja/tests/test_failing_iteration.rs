use minijinja::value::Value;
use minijinja::{context, Environment, Error, ErrorKind};

/// An iterable that yields `n` items and then fails with an invalid value.
fn failing_iterable(n: usize) -> Value {
    Value::make_iterable(move || {
        (0..=n).map(move |idx| {
            if idx == n {
                Value::from(Error::new(
                    ErrorKind::InvalidOperation,
                    "backend unavailable",
                ))
            } else {
                Value::from(idx)
            }
        })
    })
}

fn render(source: &str, items: Value) -> Result<String, Error> {
    let env = Environment::new();
    env.render_str(source, context!(items))
}

fn assert_fails(source: &str, items: Value) {
    match render(source, items) {
        Ok(rv) => panic!("expected {source:?} to fail, got {rv:?}"),
        Err(err) => {
            assert_eq!(err.kind(), ErrorKind::InvalidOperation, "{source}");
            assert_eq!(err.detail(), Some("backend unavailable"), "{source}");
        }
    }
}

#[test]
fn test_failing_iteration_in_templates() {
    for source in [
        "{% for x in items %}{{ x }}{% endfor %}",
        "{% for x in items %}{{ x }}{% else %}empty{% endfor %}",
        "{% for x in items %}{{ loop.nextitem }}{% endfor %}",
        "{{ items|list }}",
        "{{ items|join(',') }}",
        "{{ items|last }}",
        "{{ items|sort }}",
        "{{ items|reverse|list }}",
        "{{ items|unique|list }}",
        "{{ items|sum }}",
        "{{ items|min }}",
        "{{ items|max }}",
        "{{ items|map('string')|list }}",
        "{{ items|select|list }}",
        "{{ items|reject|list }}",
        "{{ items|batch(2)|list }}",
        "{{ items|slice(2)|list }}",
        "{{ items|groupby('x') }}",
        "{{ items|chain([1])|list }}",
        "{{ items|zip([1, 2, 3])|list }}",
        "{{ items[1:]|list }}",
        "{{ items[::-1]|list }}",
        "{{ items[5] }}",
        "{{ 42 in items }}",
        "{{ (items|list) + [1] }}",
        "{% set a, b, c = items %}",
    ] {
        assert_fails(source, failing_iterable(2));
    }
}

#[cfg(feature = "json")]
#[test]
fn test_failing_iteration_tojson() {
    let err = render("{{ items|tojson }}", failing_iterable(2)).unwrap_err();
    let source = std::error::Error::source(&err).unwrap();
    assert!(
        source.to_string().contains("backend unavailable"),
        "{source}"
    );
}

#[test]
fn test_failing_iteration_is_lazy() {
    // only the items that are needed are consumed, so these do not fail
    assert_eq!(
        render("{{ items|first }}", failing_iterable(2)).unwrap(),
        "0"
    );
    assert_eq!(render("{{ items[1] }}", failing_iterable(2)).unwrap(), "1");
    assert_eq!(
        render("{{ 1 in items }}", failing_iterable(2)).unwrap(),
        "True"
    );
    assert_eq!(
        render("{{ items[:2]|list }}", failing_iterable(2)).unwrap(),
        "[0, 1]"
    );
}

#[test]
fn test_checked_iteration_api() {
    let items = failing_iterable(2);
    let mut iter = items.try_iter().unwrap().checked();
    assert_eq!(iter.next().unwrap().unwrap(), Value::from(0));
    assert_eq!(iter.next().unwrap().unwrap(), Value::from(1));
    let err = iter.next().unwrap().unwrap_err();
    assert_eq!(err.detail(), Some("backend unavailable"));
    assert!(iter.next().is_none());

    let rv: Result<Vec<Value>, Error> = items.try_iter().unwrap().checked().collect();
    assert_eq!(rv.unwrap_err().detail(), Some("backend unavailable"));
}
