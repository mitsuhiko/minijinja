#![cfg(feature = "builtins")]
use minijinja::value::{Kwargs, Rest, StringInput, Value, ValueKind, ValueOrKwargs};
use minijinja::{args, context, Environment};
use similar_asserts::assert_eq;

use minijinja::filters::abs;
use minijinja::{escape_formatter, AutoEscape};

#[test]
fn test_filter_with_non() {
    fn filter(value: Option<String>) -> String {
        format!("[{}]", value.unwrap_or_default())
    }

    let mut env = Environment::new();
    env.add_filter("filter", filter);
    let state = env.empty_state();

    let rv = state
        .apply_filter("filter", args!(Value::UNDEFINED))
        .unwrap();
    assert_eq!(rv, Value::from("[]"));

    let rv = state
        .apply_filter("filter", args!(Value::from(())))
        .unwrap();
    assert_eq!(rv, Value::from("[]"));

    let rv = state
        .apply_filter("filter", args!(Value::from("wat")))
        .unwrap();
    assert_eq!(rv, Value::from("[wat]"));
}

#[test]
fn test_dotted_filter_name() {
    let mut env = Environment::new();
    env.add_filter("foo.bar.baz", |value: String| format!("<{value}>"));

    let rv = env
        .template_from_str("{{ 'hello'|foo.bar.baz }}")
        .unwrap()
        .render(context!())
        .unwrap();
    assert_eq!(rv, "<hello>");

    let rv = env
        .template_from_str("{{ 'hello'|foo . bar . baz }}")
        .unwrap()
        .render(context!())
        .unwrap();
    assert_eq!(rv, "<hello>");
}

#[test]
fn test_kwargs_require_explicit_argument_type() {
    let mut env = Environment::new();
    env.add_filter(
        "optional_value",
        |_value: Value, optional: Option<Value>| optional.unwrap_or_default(),
    );
    assert!(env
        .render_str("{{ 1|optional_value(unexpected=2) }}", ())
        .is_err());

    env.add_function("accept_kwargs", |args: Rest<ValueOrKwargs>| {
        args.last().is_some_and(|value| value.is_kwargs())
    });
    assert_eq!(
        env.render_str("{{ accept_kwargs(answer=42) }}", ())
            .unwrap(),
        "True"
    );
}

#[test]
fn test_indent() {
    let env = Environment::new();
    let state = env.empty_state();

    for (input, first, blank, expected) in [
        ("\n", None, None, ""),
        ("test\n", None, None, "test"),
        (
            "test\ntest1\n\ntest2\n",
            None,
            None,
            "test\n  test1\n\n  test2",
        ),
        (
            "test\ntest1\n\ntest2\n",
            Some(true),
            None,
            "  test\n  test1\n\n  test2",
        ),
        (
            "test\ntest1\n\ntest2\n",
            None,
            Some(true),
            "test\n  test1\n  \n  test2",
        ),
        (
            "test\ntest1\n\ntest2\n",
            Some(true),
            Some(true),
            "  test\n  test1\n  \n  test2",
        ),
    ] {
        let input = Value::from(input);
        let result = minijinja::filters::indent(
            StringInput::new(&state, &input).unwrap(),
            Some(2),
            first,
            blank,
            Kwargs::from_iter([] as [(&str, Value); 0]),
        )
        .unwrap();
        assert_eq!(result.as_str(), Some(expected));
    }
}

#[test]
fn test_indent_preserves_safe_input() {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::Html);

    let state = env.empty_state();
    let safe = state
        .apply_filter(
            "indent",
            args!(Value::from_safe_string("<p>one</p>".into()), 2),
        )
        .unwrap();
    assert!(safe.is_safe());

    let unsafe_value = state
        .apply_filter("indent", args!(Value::from("<p>one</p>"), 2))
        .unwrap();
    assert!(!unsafe_value.is_safe());

    let tmpl = env
        .template_from_str("{% filter indent(2) %}<p>one</p>\n<p>two</p>{% endfilter %}")
        .unwrap();
    assert_eq!(tmpl.render(context!()).unwrap(), "<p>one</p>\n  <p>two</p>");
}

#[test]
fn test_string_filters_preserve_safety() {
    let env = Environment::new();
    let state = env.empty_state();
    let safe = Value::from_safe_string(" Hello\nWorld ".into());
    let plain = Value::from(" Hello\nWorld ");

    for filter in ["upper", "lower", "capitalize", "reverse", "trim", "last"] {
        assert!(state
            .apply_filter(filter, args!(safe.clone()))
            .unwrap()
            .is_safe());
        assert!(!state
            .apply_filter(filter, args!(plain.clone()))
            .unwrap()
            .is_safe());
    }

    for filter in ["split", "lines"] {
        let result = state.apply_filter(filter, args!(safe.clone())).unwrap();
        assert!(result.try_iter().unwrap().all(|item| item.is_safe()));

        let result = state.apply_filter(filter, args!(plain.clone())).unwrap();
        assert!(result.try_iter().unwrap().all(|item| !item.is_safe()));
    }
}

#[test]
fn test_byte_string_filter_semantics() {
    let env = Environment::new();
    let state = env.empty_state();

    let reversed = state
        .apply_filter(
            "reverse",
            args!(Value::from_bytes("éa".as_bytes().to_vec())),
        )
        .unwrap();
    assert_eq!(reversed.kind(), ValueKind::Bytes);
    assert_eq!(reversed.as_bytes(), Some(&[b'a', 0xa9, 0xc3][..]));

    let bytes = Value::from_bytes(b"a\xffb\nc".to_vec());
    for filter in ["split", "lines"] {
        let items = state
            .apply_filter(filter, args!(bytes.clone()))
            .unwrap()
            .try_iter()
            .unwrap()
            .map(|item| item.to_string())
            .collect::<Vec<_>>();
        assert_eq!(items, ["a�b", "c"]);
    }
}

#[test]
fn test_composing_filters_escape_unsafe_fragments() {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::Html);

    let template = env
        .template_from_str(
            "{{ source|replace('x', replacement) }}|\
             {{ values|join(delimiter) }}|\
             {{ format_string|format(user) }}",
        )
        .unwrap();
    let output = template
        .render(context! {
            source => Value::from_safe_string("<b>x</b>".into()),
            replacement => "<i>y</i>",
            values => vec![
                Value::from_safe_string("<b>a</b>".into()),
                Value::from("<i>b</i>"),
            ],
            delimiter => "<hr>",
            format_string => Value::from_safe_string("<b>%s</b>".into()),
            user => "<i>x</i>",
        })
        .unwrap();

    assert_eq!(
        output,
        "<b>&lt;i&gt;y&lt;&#x2f;i&gt;</b>|\
         <b>a</b>&lt;hr&gt;&lt;i&gt;b&lt;&#x2f;i&gt;|\
         <b>&lt;i&gt;x&lt;&#x2f;i&gt;</b>"
    );

    let state = template.new_state();
    let result = state
        .apply_filter(
            "replace",
            args!(
                Value::from("<b>TEXT</b>"),
                "TEXT",
                Value::from_safe_string("<i>y</i>".into())
            ),
        )
        .unwrap();
    assert!(result.is_safe());
    assert_eq!(result.as_str(), Some("&lt;b&gt;<i>y</i>&lt;&#x2f;b&gt;"));

    let result = state
        .apply_filter(
            "join",
            args!(
                vec![Value::from("<b>a</b>"), Value::from("<i>b</i>")],
                Value::from_safe_string("<hr>".into())
            ),
        )
        .unwrap();
    assert!(result.is_safe());
    assert_eq!(
        result.as_str(),
        Some("&lt;b&gt;a&lt;&#x2f;b&gt;<hr>&lt;i&gt;b&lt;&#x2f;i&gt;")
    );

    for (filter, args) in [
        (
            "replace",
            args!(Value::from("<b>x</b>"), "x", Value::from("y")).to_vec(),
        ),
        (
            "join",
            args!(vec![Value::from("<b>a</b>")], Value::from(",")).to_vec(),
        ),
        (
            "format",
            args!(
                Value::from("<b>%s</b>"),
                Value::from_safe_string("<i>x</i>".into())
            )
            .to_vec(),
        ),
    ] {
        assert!(!state.apply_filter(filter, &args).unwrap().is_safe());
    }
}

#[test]
fn test_join_streams_when_safety_is_known() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use minijinja::value::Object;

    #[derive(Debug)]
    struct DropSignal(Arc<AtomicBool>);

    impl Object for DropSignal {}

    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn make_iterable() -> Value {
        Value::make_iterable(|| {
            let dropped = Arc::new(AtomicBool::new(false));
            let mut index = 0;
            std::iter::from_fn(move || {
                let item = match index {
                    0 => Value::from_object(DropSignal(dropped.clone())),
                    1 => {
                        assert!(dropped.load(Ordering::SeqCst));
                        Value::from("done")
                    }
                    _ => return None,
                };
                index += 1;
                Some(item)
            })
        })
    }

    let env = Environment::new();
    let state = env.empty_state();
    state
        .apply_filter("join", args!(make_iterable(), ","))
        .unwrap();

    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::Html);
    let template = env.template_from_str("").unwrap();
    let state = template.new_state();
    state
        .apply_filter(
            "join",
            args!(make_iterable(), Value::from_safe_string(",".into())),
        )
        .unwrap();
}

#[test]
fn test_safe_format_escapes_without_autoescape() {
    let env = Environment::new();
    let state = env.empty_state();
    let result = state
        .apply_filter(
            "format",
            args!(
                Value::from_safe_string("<b>%(user)s</b>".into()),
                context!(user => "<i>x</i>")
            ),
        )
        .unwrap();
    assert!(result.is_safe());
    assert_eq!(result.as_str(), Some("<b>&lt;i&gt;x&lt;&#x2f;i&gt;</b>"));

    let result = state
        .apply_filter("format", args!(Value::from_safe_string("%d".into()), 42))
        .unwrap();
    assert!(result.is_safe());
    assert_eq!(result.as_str(), Some("42"));

    let result = state
        .apply_filter(
            "format",
            args!(Value::from_safe_string("%.5s".into()), "<é"),
        )
        .unwrap();
    assert!(result.is_safe());
    assert_eq!(result.as_str(), Some("&lt;é"));

    let error = state
        .apply_filter("format", args!(Value::from_safe_string("%c".into()), 60))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("character formatting is not supported for safe format strings"));
}

#[test]
fn test_abs_overflow() {
    let ok = abs(Value::from(i64::MIN)).unwrap();
    assert_eq!(ok, Value::from(-(i64::MIN as i128)));
    let err = abs(Value::from(i128::MIN)).unwrap_err();
    assert_eq!(err.to_string(), "invalid operation: overflow on abs");
}

#[test]
fn test_chain_lists() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2] | chain([3, 4]) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[1, 2, 3, 4]");
}

#[test]
fn test_chain_length() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2] | chain([3, 4, 5]) | length }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "5");
}

#[test]
fn test_chain_dicts() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ {'a': 1} | chain({'b': 2}) | items | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[('a', 1), ('b', 2)]");
}

#[test]
fn test_chain_dict_lookup() {
    let env = Environment::new();
    // Last dict wins for lookups
    let tmpl = env
        .template_from_str("{{ ({'a': 1} | chain({'a': 2}))['a'] }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "2");
}

#[test]
fn test_chain_multiple() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1] | chain([2], [3, 4]) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[1, 2, 3, 4]");
}

#[test]
fn test_chain_with_iteration() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{% for item in [1, 2] | chain([3, 4]) %}{{ item }}{% endfor %}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "1234");
}

#[test]
fn test_chain_indexing() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ ([1, 2] | chain([3, 4]))[2] }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "3");
}

#[test]
fn test_zip_basic() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip(['a', 'b', 'c']) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[(1, 'a'), (2, 'b'), (3, 'c')]");
}

#[test]
fn test_zip_different_lengths() {
    let env = Environment::new();
    // Should stop at the shortest iterable
    let tmpl = env
        .template_from_str("{{ [1, 2] | zip(['a', 'b', 'c']) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[(1, 'a'), (2, 'b')]");
}

#[test]
fn test_zip_multiple_iterables() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip(['a', 'b', 'c'], ['x', 'y', 'z']) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[(1, 'a', 'x'), (2, 'b', 'y'), (3, 'c', 'z')]");
}

#[test]
fn test_zip_with_iteration() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{% for num, letter in [1, 2, 3] | zip(['a', 'b', 'c']) %}{{ num }}{{ letter }}{% endfor %}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "1a2b3c");
}

#[test]
fn test_zip_empty_list() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [] | zip([1, 2, 3]) | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[]");
}

#[test]
fn test_zip_non_iterable_error() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip(42) | list }}")
        .unwrap();
    let err = tmpl.render(context!()).unwrap_err();
    assert!(err
        .to_string()
        .contains("zip filter argument must be iterable"));
}

#[test]
fn test_zip_single_iterable() {
    let env = Environment::new();
    // Zip with no additional arguments should return list of single-element tuples
    let tmpl = env
        .template_from_str("{{ [1, 2, 3] | zip() | list }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[(1,), (2,), (3,)]");
}

#[test]
fn test_sort_attribute_list() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str(
            r"{{ [{'a': 1, 'b': 2, 'c': 5}, {'a': 2, 'b': 1, 'c': 6}] | sort(attribute='b,a') }}",
        )
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(
        result,
        "[{'a': 2, 'b': 1, 'c': 6}, {'a': 1, 'b': 2, 'c': 5}]"
    );
}

#[test]
fn test_sort_attribute_list_reverse() {
    let env = Environment::new();
    let ctx = context! {
        cities => vec![
            context!(name => "Sydney", country => "Australia"),
            context!(name => "Sydney", country => "Canada"),
            context!(name => "Kochi", country => "India"),
            context!(name => "Kochi", country => "Japan"),
        ]
    };
    let tmpl = env
        .template_from_str(
            "{{ cities | sort(attribute='name, country', reverse=true) \
             | map(attribute='country')}}",
        )
        .unwrap();
    let result = tmpl.render(ctx).unwrap();
    assert_eq!(result, "['Canada', 'Australia', 'Japan', 'India']");
}

#[test]
fn test_sort_attribute_list_single() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str(r"{{ [{'a': 1, 'b': 2}, {'a': 2, 'b': 1}] | sort(attribute='b,') }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "[{'a': 2, 'b': 1}, {'a': 1, 'b': 2}]");
}

#[test]
fn test_sort_stable_reverse() {
    let env = Environment::new();

    // Test sorting a list of 2-tuples using 0th element as the key, in reverse
    // order. The sort should be stable and preserve the relative order of the
    // equivalent keys.
    let test_cases = [
        ("[[1, 2], [1, 3], [1, 4]]", "[[1, 2], [1, 3], [1, 4]]"),
        ("[[1, 2], [1, 3], [2, 4]]", "[[2, 4], [1, 2], [1, 3]]"),
        ("[[3, 1], [2, 2], [1, 3]]", "[[3, 1], [2, 2], [1, 3]]"),
        ("[[3, 3], [2, 2], [1, 1]]", "[[3, 3], [2, 2], [1, 1]]"),
        ("[[1, 2], [2, 2], [3, 2]]", "[[3, 2], [2, 2], [1, 2]]"),
        (
            "[[1, 2], [3, 3], [2, 4], [3, 4]]",
            "[[3, 3], [3, 4], [2, 4], [1, 2]]",
        ),
        (
            "[[1, 2], [2, 2], [3, 2], [1, 1], [2, 2], [3, 3]]",
            "[[3, 2], [3, 3], [2, 2], [2, 2], [1, 2], [1, 1]]",
        ),
    ];

    for (input, expected) in test_cases {
        let stmt = format!("{{{{ {input} | sort(attribute='0', reverse=true) }}}}");
        let tmpl = env.template_from_str(&stmt).unwrap();
        let result = tmpl.render(context!()).unwrap();
        assert_eq!(result, expected);
    }

    // Test stable reverse-sorting with multi-attribute key.
    let tmpl = env
        .template_from_str(
            "{{ [{'a': 1, 'b': 1, 'c': 1}, {'a': 1, 'b': 1, 'c': 2}] \
             | sort(attribute='a,b', reverse=true) }}",
        )
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(
        result,
        "[{'a': 1, 'b': 1, 'c': 1}, {'a': 1, 'b': 1, 'c': 2}]"
    );
}

#[test]
fn test_sort_strings() {
    let env = Environment::new();

    let tmpl = env
        .template_from_str("{{ ['aa', 'CC', 'bb'] | sort }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "['aa', 'bb', 'CC']");

    let tmpl = env
        .template_from_str("{{ ['aa', 'CC', 'bb'] | sort(reverse=True) }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "['CC', 'bb', 'aa']");

    let tmpl = env
        .template_from_str("{{ ['aa', 'CC', 'bb'] | sort(case_sensitive=True) }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "['CC', 'aa', 'bb']");

    let tmpl = env
        .template_from_str("{{ ['aa', 'CC', 'bb'] | sort(case_sensitive=True, reverse=True) }}")
        .unwrap();
    let result = tmpl.render(context!()).unwrap();
    assert_eq!(result, "['bb', 'aa', 'CC']");
}

#[test]
#[cfg(feature = "json")]
fn test_tojson_uses_jinja_spacing() {
    let env = Environment::new();
    let tmpl = env
        .template_from_str(
            r#"{{ {"function": {"name": "get_weather", "parameters": {"type": "object"}}, "type": "function"}|tojson }}"#,
        )
        .unwrap();
    assert_eq!(
        tmpl.render(()).unwrap(),
        r#"{"function": {"name": "get_weather", "parameters": {"type": "object"}}, "type": "function"}"#
    );
}

#[test]
fn test_escape_filter_custom_formatter() {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::Custom("Markdown"));
    env.set_formatter(|out, state, value| {
        if value.is_safe() && value.kind() == ValueKind::String {
            return out
                .write_str(value.as_str().unwrap_or_default())
                .map_err(minijinja::Error::from);
        }

        match state.auto_escape() {
            AutoEscape::Custom("Markdown") => {
                let escaped = value.to_string().replace('*', "\\*");
                write!(out, "{escaped}").map_err(minijinja::Error::from)
            }
            _ => escape_formatter(out, state, value),
        }
    });

    let tmpl = env.template_from_str("{{ value|e }}").unwrap();
    let result = tmpl.render(context! { value => "*" }).unwrap();
    assert_eq!(result, "\\*");

    let tmpl = env
        .template_from_str("{% autoescape false %}{{ value|e }}{% endautoescape %}")
        .unwrap();
    let result = tmpl.render(context! { value => "*" }).unwrap();
    assert_eq!(result, "\\*");

    let tmpl = env
        .template_from_str("{{ '%s'|safe|format(value) }}|{{ values|join('*') }}")
        .unwrap();
    let result = tmpl
        .render(context! {
            value => "*",
            values => vec![
                Value::from_safe_string("ok".into()),
                Value::from("end"),
            ],
        })
        .unwrap();
    assert_eq!(result, "\\*|ok\\*end");
}
