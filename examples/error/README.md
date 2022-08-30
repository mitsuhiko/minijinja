# error

This example demonstrates the built-in error rendering support for better
template debuggability

```console
$ cargo run
Template Failed Rendering:
  impossible operation: tried to use + operator on unsupported types number and string (in hello.txt:8)
---------------------------- Template Source -----------------------------
   5 |             {% with foo = 42 %}
   6 |               {{ range(10) }}
   7 |               {{ other_seq|join(" ") }}
   8 >               Hello {{ item_squared + bar }}!
   9 |             {% endwith %}
  10 |           {% endwith %}
  11 |         {% endfor %}
--------------------------------------------------------------------------
Referenced variables: {
    bar: "test",
    item_squared: 4,
    other_seq: [...],
    range: minijinja::functions::builtins::range,
    foo: 42,
}
--------------------------------------------------------------------------
```
